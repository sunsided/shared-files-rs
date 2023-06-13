use crate::{Sentinel, SharedFileType, WriteState};
use pin_project::{pin_project, pinned_drop};
use std::io::{Error, ErrorKind, SeekFrom};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io;
use tokio::io::{AsyncRead, AsyncSeek, ReadBuf};
use uuid::Uuid;

/// A reader for the shared temporary file.
#[pin_project(PinnedDrop)]
pub struct SharedFileReader<T> {
    /// The ID of the reader.
    id: Uuid,
    /// The file to read from.
    #[pin]
    file: T,
    /// The sentinel value to keep the file alive.
    sentinel: Arc<Sentinel<T>>,
}

/// These IDs never leave the current system, so the node ID is arbitrary.
static NODE_ID: &'static [u8; 6] = &[2, 3, 0, 6, 1, 2];

impl<T> SharedFileReader<T>
where
    T: SharedFileType<Type = T>,
{
    pub(crate) fn new(file: T, sentinel: Arc<Sentinel<T>>) -> Self {
        Self {
            id: Uuid::now_v1(&NODE_ID),
            file,
            sentinel,
        }
    }

    /// Creates a new, independent reader.
    pub async fn fork(&self) -> Result<Self, T::OpenError> {
        Ok(Self {
            id: Uuid::now_v1(&NODE_ID),
            file: self.sentinel.original.open_ro().await?,
            sentinel: self.sentinel.clone(),
        })
    }
}

impl<T> SharedFileReader<T> {
    /// Gets the (expected) size of the file to read.
    pub fn file_size(&self) -> FileSize {
        match self.sentinel.state.load() {
            WriteState::Pending(size) => FileSize::AtLeast(size),
            WriteState::Completed(size) => FileSize::Exactly(size),
            WriteState::Failed => FileSize::Error,
        }
    }
}

/// The file size of the file to read.
#[derive(Debug, Copy, Clone)]
pub enum FileSize {
    /// The file is not entirely written yet. The specified amount is the minimum
    /// number known to exist.
    AtLeast(usize),
    /// The file is completely written and has exactly the specified amount of bytes.
    Exactly(usize),
    /// An error occurred while writing the file; reading may not complete.
    Error,
}

#[pinned_drop]
impl<T> PinnedDrop for SharedFileReader<T> {
    fn drop(mut self: Pin<&mut Self>) {
        self.sentinel.remove_reader_waker(&self.id)
    }
}

impl<T> AsyncRead for SharedFileReader<T>
where
    T: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let pos = buf.filled().len();

        // TODO: Ensure to not read more bytes than were actually written.
        //       This should help dealing with pre-allocated files.
        if let Poll::Ready(result) = this.file.poll_read(cx, buf) {
            this.sentinel.remove_reader_waker(&this.id);
            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }

            // If the buffer was advanced, return the result.
            let buf_fill = buf.filled().len();
            if buf_fill != pos {
                return Poll::Ready(result);
            }

            // If the buffer was not advanced and source file is completed (or in fail state),
            // return as-is. Otherwise, reset and wait.
            match this.sentinel.state.load() {
                WriteState::Pending(_) => {}
                WriteState::Completed(_) => return Poll::Ready(Ok(())),
                WriteState::Failed => {
                    return Poll::Ready(Err(Error::new(
                        ErrorKind::BrokenPipe,
                        ReadError::FileClosed,
                    )))
                }
            }
        }

        // Re-register waker and try again.
        this.sentinel
            .register_reader_waker(this.id.clone(), cx.waker());
        Poll::Pending
    }
}

impl<T> AsyncSeek for SharedFileReader<T>
where
    T: AsyncSeek,
{
    fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
        let this = self.project();
        this.file.start_seek(position)
    }

    fn poll_complete(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<u64>> {
        let this = self.project();
        this.file.poll_complete(cx)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Io(#[from] Error),
    #[error("The file was already closed")]
    FileClosed,
}
