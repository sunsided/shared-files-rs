use crate::{Sentinel, SharedFileType, WriteState};
use pin_project::{pin_project, pinned_drop};
use std::io::{Error, ErrorKind, SeekFrom};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// The number of bytes read. Used to keep track
    /// of how many bytes need to be read from the underlying buffer.
    read: AtomicUsize,
}

/// These IDs never leave the current system, so the node ID is arbitrary.
static NODE_ID: &[u8; 6] = &[2, 3, 0, 6, 1, 2];

impl<T> SharedFileReader<T>
where
    T: SharedFileType<Type = T>,
{
    pub(crate) fn new(file: T, sentinel: Arc<Sentinel<T>>) -> Self {
        Self {
            id: Uuid::now_v1(NODE_ID),
            file,
            sentinel,
            read: AtomicUsize::new(0),
        }
    }

    /// Creates a new, independent reader.
    pub async fn fork(&self) -> Result<Self, T::OpenError> {
        Ok(Self {
            id: Uuid::now_v1(NODE_ID),
            file: self.sentinel.original.open_ro().await?,
            sentinel: self.sentinel.clone(),
            read: AtomicUsize::new(0),
        })
    }
}

impl<T> SharedFileReader<T> {
    /// Gets the (expected) size of the file to read.
    pub fn file_size(&self) -> FileSize {
        match self.sentinel.state.load() {
            WriteState::Pending(commited, _written) => FileSize::AtLeast(commited),
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
        let read_so_far = self.read.load(Ordering::Acquire);

        let current_total = match self.sentinel.state.load() {
            WriteState::Pending(committed, _written) => {
                // If the number of committed bytes is the same as the number
                // of bytes we have already read, try again later.
                if read_so_far == committed {
                    self.sentinel.register_reader_waker(self.id, cx.waker());
                    return Poll::Pending;
                }
                committed
            }
            WriteState::Completed(count) => {
                // If we have read all there is, we're done.
                if read_so_far == count {
                    return Poll::Ready(Ok(()));
                }
                count
            }
            WriteState::Failed => {
                return Poll::Ready(Err(Error::new(
                    ErrorKind::BrokenPipe,
                    ReadError::FileClosed,
                )))
            }
        };

        // Ensure to not read more bytes than were actually written
        // by constraining the actual buffer to a smaller one if needed.
        let read_at_most = (current_total - read_so_far).min(buf.remaining());
        let mut smaller_buf = buf.take(read_at_most);
        let read_offset = smaller_buf.filled().len();

        let this = self.project();

        if let Poll::Ready(result) = this.file.poll_read(cx, &mut smaller_buf) {
            this.sentinel.remove_reader_waker(this.id);
            if let Err(e) = result {
                return Poll::Ready(Err(e));
            }

            // If the buffer was advanced, return the result.
            let read_now = smaller_buf.filled().len();
            if read_now != read_offset {
                // Advance the parent buffer.
                unsafe {
                    buf.assume_init(read_now);
                }
                buf.set_filled(read_now);

                let read = read_so_far + (read_now - read_offset);
                this.read.store(read, Ordering::Release);
                return Poll::Ready(result);
            }

            // If the buffer was not advanced and source file is completed (or in fail state),
            // return as-is. Otherwise, reset and wait.
            match this.sentinel.state.load() {
                WriteState::Pending(_, _) => {}
                WriteState::Completed(_) => return Poll::Ready(Ok(())),
                WriteState::Failed => {
                    return Poll::Ready(Err(Error::new(
                        ErrorKind::BrokenPipe,
                        ReadError::FileClosed,
                    )))
                }
            }
        }

        // "Advance" the parent buffer.
        buf.advance(0);

        // Re-register waker and try again.
        this.sentinel.register_reader_waker(*this.id, cx.waker());
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
