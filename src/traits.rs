use std::path::PathBuf;
use tokio::io::{AsyncRead, AsyncWrite};

/// Trait for types used as a file storage backend.
#[async_trait::async_trait]
pub trait SharedFileType: AsyncRead + AsyncWrite + Unpin {
    /// The type created when producing a reader or writer. Typically `Self`.
    type Type;

    /// The error type.
    type OpenError;

    /// The error type.
    type SyncError;

    /// Opens a new [`Type`](Self::Type) instance in read-only mode.
    async fn open_ro(&self) -> Result<Self::Type, Self::OpenError>;

    /// Opens a new [`Type`](Self::Type) instance in read-write mode.
    async fn open_rw(&self) -> Result<Self::Type, Self::OpenError>;

    /// Synchronizes data and metadata with the underlying buffer.
    async fn sync_all(&self) -> Result<(), Self::SyncError>;

    /// Synchronizes data with the underlying buffer.
    async fn sync_data(&self) -> Result<(), Self::SyncError>;
}

/// Trait for types that can be newly constructed asynchronously.
#[async_trait::async_trait]
pub trait AsyncNewFile {
    /// The type created on success.
    type Target;
    /// The error type.
    type Error;

    /// Creates a new instance of the type [`Target`](AsyncNewFile::Target).
    async fn new() -> Result<Self::Target, Self::Error>;
}

/// Trait for types that can be newly constructed asynchronously.
pub trait NewFile {
    /// The type created on success.
    type Target;
    /// The error type.
    type Error;

    /// Creates a new instance of the type [`Target`](AsyncNewFile::Target).
    fn new() -> Result<Self::Target, Self::Error>;
}

/// Trait for types that can synchronously determine the file path.
pub trait FilePath {
    /// Obtains the path of the temporary file.
    fn file_path(&self) -> &PathBuf;
}
