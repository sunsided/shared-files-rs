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
//! The functionality is currently based on the [async-tempfile](https://github.com/sunsided/async-tempfile-rs)
//! crate. A generic implementation is planned for the use of arbitrary `AsyncWrite` /
//! `AsyncRead` backing.

#![forbid(unsafe_code)]

mod reader;
mod writer;

use async_tempfile::TempFile;
use crossbeam::atomic::AtomicCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use uuid::Uuid;

pub use reader::{FileSize, ReadError, SharedTemporaryFileReader};
pub use writer::{CompleteWritingError, SharedTemporaryFileWriter, WriteError};

/// A temporary file with shared read/write access.
pub struct SharedTemporaryFile {
    /// The sentinel value to keep the file alive.
    sentinel: Arc<Sentinel>,
}

struct Sentinel {
    /// The original file. This keeps the file open until all references are dropped.
    original: TempFile,
    /// The state of the write operation.
    state: AtomicCell<State>,
    /// Wakers to wake up all interested readers.
    wakers: Mutex<HashMap<Uuid, Waker>>,
}

#[derive(Debug, Clone, Copy)]
enum State {
    /// The write operation is pending. Contains the number of bytes written.
    Pending(usize),
    /// The write operation completed. Contains the file size.
    Completed(usize),
    /// The write operation failed.
    Failed,
}

impl SharedTemporaryFile {
    /// Creates a new temporary file.
    pub async fn new() -> Result<SharedTemporaryFile, async_tempfile::Error> {
        let file = TempFile::new().await?;
        Ok(Self {
            sentinel: Arc::new(Sentinel {
                original: file,
                state: AtomicCell::new(State::Pending(0)),
                wakers: Mutex::new(HashMap::default()),
            }),
        })
    }

    /// Obtains the path of the temporary file.
    pub async fn file_path(&self) -> &PathBuf {
        self.sentinel.original.file_path()
    }

    /// Creates a writer for the file.
    ///
    /// Note that this operation can result in odd behavior if the
    /// file is accessed multiple times for write access. User code
    /// must make sure that only one meaningful write is performed at
    /// the same time.
    pub async fn writer(&self) -> Result<SharedTemporaryFileWriter, async_tempfile::Error> {
        let file = self.sentinel.original.open_rw().await?;
        Ok(SharedTemporaryFileWriter::new(file, self.sentinel.clone()))
    }

    /// Creates a reader for the file.
    pub async fn reader(&self) -> Result<SharedTemporaryFileReader, async_tempfile::Error> {
        let file = self.sentinel.original.open_ro().await?;
        Ok(SharedTemporaryFileReader::new(file, self.sentinel.clone()))
    }
}

impl Sentinel {
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
            .and_modify(|e| *e = waker.clone())
            .or_insert(waker.clone());
    }

    fn remove_reader_waker(&self, id: &Uuid) {
        let mut lock = self.wakers.lock().expect("failed to get lock for readers");
        lock.remove(id);
    }
}
