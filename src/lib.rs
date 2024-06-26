//! # Disk-Based Single-Writer, Multiple-Reader In-Process File Sharing
//!
//! Functionality for single-writer, multiple-reader file operations where multiple concurrent
//! readers need to read from a file that is currently being written by the same process. The
//! intended use case is the parallel processing of byte streams with minimum (process) memory
//! requirements, e.g. in web services moving around large files.
//!
//! Normally, reading a file while it is written results in the read stream ending prematurely
//! as EOF; the purpose of this crate is to prevent exactly that.
//!
//! Any file type can be used as a backing as long as it implements the crate's [`SharedFileType`]
//! trait, which in turn requires [`AsyncWrite`](tokio::io::AsyncWrite) and [`AsyncRead`](tokio::io::AsyncRead).
//!
//! ## Crate Features
//!
//! - `async-tempfile`: Enables the [`SharedTemporaryFile`] type via the
//!   [async-tempfile](https://github.com/sunsided/async-tempfile-rs) crate. Since this is how
//!   this crate was initially meant to be used, this feature is enabled by default.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(unsafe_code)]

mod reader;

mod errors;
#[cfg_attr(docsrs, doc(cfg(feature = "async-tempfile")))]
#[cfg(feature = "async-tempfile")]
mod temp_file;
mod traits;
mod writer;

use crossbeam::atomic::AtomicCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use uuid::Uuid;

pub use reader::{FileSize, SharedFileReader};
pub use traits::*;
pub use writer::SharedFileWriter;

/// Prelude for commonly used types and traits.
pub mod prelude {
    pub use crate::errors::*;
    pub use crate::traits::*;
    pub use crate::SharedFile;

    #[cfg_attr(docsrs, doc(cfg(feature = "async-tempfile")))]
    #[cfg(feature = "async-tempfile")]
    pub use crate::SharedTemporaryFile;
}

#[cfg_attr(docsrs, doc(cfg(feature = "async-tempfile")))]
#[cfg(feature = "async-tempfile")]
pub use temp_file::*;

/// A file with shared read/write access for in-process file sharing.
///
/// ## Writer / Reader Synchronization
/// Since this wrapper takes over control of the write operation, readers
/// will only be woken up on a call to [`SharedFileWriter::sync_data`],
/// [`SharedFileWriter::sync_all`] or [`SharedFileWriter::flush`]. This is to
/// ensure that data is actually written to the underlying buffer before
/// the readers attempt to read it back.
///
/// ## Writer Finalization
/// When a writer is dropped, it will move the state of the [`SharedFile`] to
/// [`WriteState::Completed`]. It is important to note that drop is not asynchronous
/// and therefore no flush to disk can be performed on the wrapped file.
///
/// <div class="warning">User code must make sure to manually sync to disk before dropping the writer.</div>
#[derive(Debug)]
pub struct SharedFile<T> {
    /// The sentinel value to keep the file alive.
    sentinel: Arc<Sentinel<T>>,
}

#[derive(Debug)]
struct Sentinel<T> {
    /// The original file. This keeps the file open until all references are dropped.
    original: T,
    /// The state of the write operation.
    state: AtomicCell<WriteState>,
    /// Wakers to wake up all interested readers.
    wakers: Mutex<HashMap<Uuid, Waker>>,
}

/// The state of a file write operation.
#[derive(Debug, Clone, Copy)]
enum WriteState {
    /// The write operation is pending. Contains the committed byte count and the written byte count.
    Pending(usize, usize),
    /// The write operation completed. Contains the number of bytes written (and committed).
    Completed(usize),
    /// The write operation failed.
    Failed,
}

impl<T> SharedFile<T>
where
    T: SharedFileType<Type = T>,
{
    /// Synchronously creates a new temporary file.
    pub fn new() -> Result<SharedFile<T>, T::Error>
    where
        T: NewFile<Target = T>,
    {
        let file = T::new()?;
        Ok(Self::from(file))
    }

    /// Asynchronously creates a new temporary file.
    pub async fn new_async() -> Result<SharedFile<T>, T::Error>
    where
        T: AsyncNewFile<Target = T>,
    {
        let file = T::new_async().await?;
        Ok(Self::from(file))
    }

    /// Creates a writer for the file.
    ///
    /// ## Reader / writer Synchronization
    ///
    /// Since this wrapper takes over control of the write operation, readers
    /// will only be woken up on a call to [`SharedFileWriter::sync_data`],
    /// [`SharedFileWriter::sync_all`] or [`SharedFileWriter::flush`]. This is to
    /// ensure that data is actually written to the underlying buffer before
    /// the readers attempt to read it back.
    ///
    /// ## Writer finalization
    ///
    /// <div class="warning">User code must make sure to manually sync to disk before dropping the writer.</div>
    ///
    /// When a writer is dropped, it will move the state of the [`SharedFile`] to
    /// [`WriteState::Completed`]. It is important to note that drop is not asynchronous
    /// and therefore no flush to disk can be performed on the wrapped file.
    ///
    /// ## One writer at a time
    ///
    /// This operation can result in odd behavior if the
    /// file is accessed multiple times for write access. User code
    /// must make sure that only one meaningful write is performed at
    /// the same time.
    pub async fn writer(&self) -> Result<SharedFileWriter<T::Type>, T::OpenError> {
        let file = self.sentinel.original.open_rw().await?;
        Ok(SharedFileWriter::new(file, self.sentinel.clone()))
    }

    /// Creates a reader for the file.
    pub async fn reader(&self) -> Result<SharedFileReader<T::Type>, T::OpenError> {
        let file = self.sentinel.original.open_ro().await?;
        Ok(SharedFileReader::new(file, self.sentinel.clone()))
    }
}

impl<T> From<T> for SharedFile<T> {
    fn from(value: T) -> Self {
        Self {
            sentinel: Arc::new(Sentinel {
                original: value,
                state: AtomicCell::new(WriteState::Pending(0, 0)),
                wakers: Mutex::new(HashMap::default()),
            }),
        }
    }
}

impl<T> Default for SharedFile<T>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            sentinel: Arc::new(Sentinel {
                original: T::default(),
                state: AtomicCell::new(WriteState::Pending(0, 0)),
                wakers: Mutex::new(HashMap::default()),
            }),
        }
    }
}

impl<T> FilePath for SharedFile<T>
where
    T: FilePath,
{
    fn file_path(&self) -> &PathBuf {
        self.sentinel.original.file_path()
    }
}

impl<T> Sentinel<T> {
    fn wake_readers(&self) {
        let mut lock = self
            .wakers
            .lock()
            .expect("failed to lock waker vector for writing");
        lock.drain().for_each(|(_id, w)| w.wake());
    }

    fn register_reader_waker(&self, id: Uuid, waker: &Waker) {
        let mut lock = self
            .wakers
            .lock()
            .expect("failed to lock waker vector for reading");

        lock.entry(id)
            .and_modify(|e| e.clone_from(waker))
            .or_insert(waker.clone());
    }

    fn remove_reader_waker(&self, id: &Uuid) {
        let mut lock = self.wakers.lock().expect("failed to get lock for readers");
        lock.remove(id);
    }
}
