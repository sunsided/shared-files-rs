use crate::{FilePath, Sentinel, SharedFileType, WriteState};
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
    pub async fn file_path(&self) -> &PathBuf
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
        self.file.sync_all().await
    }

    /// Synchronizes data with the disk buffer.
    pub async fn sync_data(&self) -> Result<(), T::SyncError>
    where
        T: SharedFileType,
    {
        self.file.sync_data().await
    }

    /// Completes the writing operation.
    ///
    /// Use [`complete_no_sync`](Self::complete_no_sync) if you do not wish
    /// to sync the file to disk.
    pub async fn complete(self) -> Result<(), CompleteWritingError>
    where
        T: SharedFileType,
    {
        if let Err(_) = self.file.sync_all().await {
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

    /// Sets the state to finalized.
    ///
    /// See also [`update_state`](Self::update_state) for increasing the byte count.
    fn finalize_state(&self) -> Result<(), CompleteWritingError> {
        let result = match self.sentinel.state.load() {
            WriteState::Pending(size) => {
                self.sentinel.state.store(WriteState::Completed(size));
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
            WriteState::Pending(count) => {
                state.store(WriteState::Pending(count + written));
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
        let result = match poll {
            Poll::Ready(result) => match result {
                Ok(written) => match Self::update_state(&sentinel.state, written) {
                    Ok(_) => Poll::Ready(Ok(written)),
                    Err(e) => Poll::Ready(Err(e)),
                },
                Err(e) => {
                    sentinel.state.store(WriteState::Failed);
                    Poll::Ready(Err(e))
                }
            },
            Poll::Pending => Poll::Pending,
        };

        // Wake up waiting futures.
        if let Poll::Ready(e) = result {
            sentinel.wake_readers();
            Poll::Ready(e)
        } else {
            Poll::Pending
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for SharedFileWriter<T> {
    fn drop(mut self: Pin<&mut Self>) {
        self.finalize_state().ok();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CompleteWritingError {
    #[error(transparent)]
    Io(#[from] Error),
    #[error("Writing to the file failed")]
    FileWritingFailed,
    #[error("Failed to synchronize the file with the underlying buffer")]
    SyncError,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error(transparent)]
    Io(#[from] Error),
    #[error("The file was already closed")]
    FileClosed,
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
        Self::handle_poll_write_result(&this.sentinel, poll)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        match this.file.poll_flush(cx) {
            Poll::Ready(result) => match result {
                Ok(()) => {
                    // Flushing doesn't change the number of bytes written,
                    // so we don't update the counter here.
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

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let this = self.project();
        match this.file.poll_shutdown(cx) {
            Poll::Ready(result) => match result {
                Ok(()) => {
                    if let WriteState::Pending(count) = this.sentinel.state.load() {
                        this.sentinel.state.store(WriteState::Completed(count));
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
        Self::handle_poll_write_result(&this.sentinel, poll)
    }

    fn is_write_vectored(&self) -> bool {
        self.file.is_write_vectored()
    }
}
