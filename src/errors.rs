use std::fmt::{Display, Formatter};
use tokio::io;

#[derive(Debug)]
pub enum CompleteWritingError {
    /// An I/O error occurred.
    Io(io::Error),
    /// Writing to the file failed.
    FileWritingFailed,
    /// Failed to synchronize the file with the underlying buffer.
    SyncError,
}

#[derive(Debug)]
pub enum WriteError {
    /// An I/O error occurred.
    Io(io::Error),
    /// The file was already closed
    FileClosed,
}

#[derive(Debug)]
pub enum ReadError {
    /// An I/O error occurred.
    Io(io::Error),
    /// The file was already closed
    FileClosed,
}

impl Display for CompleteWritingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CompleteWritingError::Io(io) => write!(f, "{}", io),
            CompleteWritingError::FileWritingFailed => write!(f, "Writing to the file failed"),
            CompleteWritingError::SyncError => write!(
                f,
                "Failed to synchronize the file with the underlying buffer"
            ),
        }
    }
}

impl Display for WriteError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WriteError::Io(io) => write!(f, "{}", io),
            WriteError::FileClosed => write!(f, "The file was already closed"),
        }
    }
}

impl Display for ReadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Io(io) => write!(f, "{}", io),
            ReadError::FileClosed => write!(f, "The file was already closed"),
        }
    }
}

impl From<io::Error> for CompleteWritingError {
    fn from(value: io::Error) -> Self {
        CompleteWritingError::Io(value)
    }
}

impl From<io::Error> for WriteError {
    fn from(value: io::Error) -> Self {
        WriteError::Io(value)
    }
}

impl From<io::Error> for ReadError {
    fn from(value: io::Error) -> Self {
        ReadError::Io(value)
    }
}

impl std::error::Error for CompleteWritingError {}
impl std::error::Error for WriteError {}
impl std::error::Error for ReadError {}
