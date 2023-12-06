//! File writing functionality, notably the [`SharedFileWriter`] type.

use crate::errors::{CompleteWritingError, WriteError};
use crate::{FilePath, Sentinel, SharedFileReader, SharedFileType, WriteState};
use crossbeam::atomic::AtomicCell;
use pin_project::{pin_project, pinned_drop};
use std::io::{Error, ErrorKind, IoSlice};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io;
use tokio::io::AsyncWrite;

/// A writer for the shared temporary file.
///
/// ## Dropping the writer
///
/// Note that while dropping the writer while implicitly change it to "completed",
/// you must manually call [`SharedFileWriter::sync_all`] or [`SharedFileWriter::sync_data`]
/// to ensure all content is flushed to the underlying buffer.
#[pin_project(PinnedDrop)]
pub struct SharedFileWriter<T> {
    /// The file to write to.
    #[pin]
    file: T,
    /// The sentinel value to keep the file alive.
    sentinel: Arc<Sentinel<T>>,
}

impl<T> SharedFileWriter<T> {
    pub(crate) fn new(file: T, sentinel: Arc<Sentinel<T>>) -> Self {
        Self { file, sentinel }
    }

    /// Gets the file path.
    pub fn file_path(&self) -> &PathBuf
    where
        T: FilePath,
    {
        self.file.file_path()
    }

    /// Synchronizes data and metadata with the disk buffer.
    pub async fn sync_all(&self) -> Result<(), T::SyncError>
    where
        T: SharedFileType,
    {
        self.file.sync_all().await?;
        Self::sync_committed_and_written(&self.sentinel);
        self.sentinel.wake_readers();
        Ok(())
    }

    /// Synchronizes data with the disk buffer.
    pub async fn sync_data(&self) -> Result<(), T::SyncError>
    where
        T: SharedFileType,
    {
        self.file.sync_data().await?;
        Self::sync_committed_and_written(&self.sentinel);
        self.sentinel.wake_readers();
        Ok(())
    }

    /// Completes the writing operation.
    ///
    /// Use [`complete_no_sync`](Self::complete_no_sync) if you do not wish
    /// to sync the file to disk.
    pub async fn complete(self) -> Result<(), CompleteWritingError>
    where
        T: SharedFileType,
    {
        if self.sync_all().await.is_err() {
            return Err(CompleteWritingError::SyncError);
        }
        self.complete_no_sync()
    }

    /// Completes the writing operation.
    ///
    /// If you need to sync the file to disk, consider calling
    /// [`complete`](Self::complete) instead.
    pub fn complete_no_sync(self) -> Result<(), CompleteWritingError> {
        self.finalize_state()
    }

    /// Synchronizes the number of written bytes with the number of committed bytes.
    fn sync_committed_and_written(sentinel: &Arc<Sentinel<T>>) {
        match sentinel.state.load() {
            WriteState::Pending(_committed, written) => {
                sentinel.state.store(WriteState::Pending(written, written));
            }
            WriteState::Completed(_) => {}
            WriteState::Failed => {}
        }
    }

    /// Sets the state to finalized.
    ///
    /// See also [`update_state`](Self::update_state) for increasing the byte count.
    fn finalize_state(&self) -> Result<(), CompleteWritingError> {
        let result = match self.sentinel.state.load() {
            WriteState::Pending(_committed, written) => {
                assert_eq!(_committed, written, "The number of committed bytes is less than the number of written bytes - call sync before dropping");
                self.sentinel.state.store(WriteState::Completed(written));
                Ok(())
            }
            WriteState::Completed(_) => Ok(()),
            WriteState::Failed => Err(CompleteWritingError::FileWritingFailed),
        };

        self.sentinel.wake_readers();
        result
    }

    /// Updates the internal byte count with the specified number of bytes written.
    /// Will produce an error if the update failed.
    ///
    /// ## Returns
    /// Returns the number of bytes written in total.
    ///
    /// See also [`finalize_state`](Self::finalize_state) for finalizing the write.
    fn update_state(state: &AtomicCell<WriteState>, written: usize) -> Result<usize, Error> {
        match state.load() {
            WriteState::Pending(committed, previously_written) => {
                let count = previously_written + written;
                state.store(WriteState::Pending(committed, count));
                Ok(count)
            }
            WriteState::Completed(count) => {
                // Ensure we do not try to write more data after completing
                // the file.
                if written != 0 {
                    return Err(Error::new(ErrorKind::BrokenPipe, WriteError::FileClosed));
                }
                Ok(count)
            }
            WriteState::Failed => Err(Error::from(ErrorKind::Other)),
        }
    }

    /// Processes a [`Poll`] result from a write operation.
    ///
    /// This will update the internal byte count and produce an error
    /// if the update failed.
    fn handle_poll_write_result(
        sentinel: &Sentinel<T>,
        poll: Poll<Result<usize, Error>>,
    ) -> Poll<Result<usize, Error>> {
        match poll {
            Poll::Ready(result) => match result {
                Ok(written) => match Self::update_state(&sentinel.state, written) {
                    Ok(_) => Poll::Ready(Ok(written)),
                    Err(e) => Poll::Ready(Err(e)),
                },
                Err(e) => {
                    sentinel.state.store(WriteState::Failed);
                    sentinel.wake_readers();
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for SharedFileWriter<T> {
    fn drop(mut self: Pin<&mut Self>) {
        self.finalize_state().ok();
    }
}

impl<T> AsyncWrite for SharedFileWriter<T>
where
    T: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        let poll = this.file.poll_write(cx, buf);
        Self::handle_poll_write_result(this.sentinel, poll)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.file.poll_flush(cx) {
            Poll::Ready(result) => match result {
                Ok(()) => {
                    Self::sync_committed_and_written(this.sentinel);
                    this.sentinel.wake_readers();
                    Poll::Ready(Ok(()))
                }
                Err(e) => {
                    this.sentinel.state.store(WriteState::Failed);
                    this.sentinel.wake_readers();
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.project();
        match this.file.poll_shutdown(cx) {
            Poll::Ready(result) => match result {
                Ok(()) => {
                    if let WriteState::Pending(_committed, written) = this.sentinel.state.load() {
                        debug_assert_eq!(_committed, written);
                        this.sentinel.state.store(WriteState::Completed(written));
                    }

                    Poll::Ready(Ok(()))
                }
                Err(e) => {
                    this.sentinel.state.store(WriteState::Failed);
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, Error>> {
        let this = self.project();
        let poll = this.file.poll_write_vectored(cx, bufs);
        Self::handle_poll_write_result(this.sentinel, poll)
    }

    fn is_write_vectored(&self) -> bool {
        self.file.is_write_vectored()
    }
}
