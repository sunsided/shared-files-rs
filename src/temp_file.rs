use crate::{
    AsyncNewFile, CompleteWritingError, FilePath, SharedFile, SharedFileReader, SharedFileType,
    SharedFileWriter,
};
use async_tempfile::{Ownership, TempFile};
use std::ops::Deref;
use std::path::PathBuf;
use tokio::fs::File;

#[async_trait::async_trait]
impl SharedFileType for TempFile {
    type Type = TempFile;
    type OpenError = async_tempfile::Error;
    type SyncError = CompleteWritingError;

    async fn open_ro(&self) -> Result<Self::Type, Self::OpenError> {
        self.open_ro().await
    }

    async fn open_rw(&self) -> Result<Self::Type, Self::OpenError> {
        self.open_rw().await
    }

    async fn sync_all(&self) -> Result<(), Self::SyncError> {
        let file: &File = self.deref();
        Ok(file.sync_all().await?)
    }

    async fn sync_data(&self) -> Result<(), Self::SyncError> {
        let file: &File = self.deref();
        Ok(file.sync_data().await?)
    }
}

#[async_trait::async_trait]
impl AsyncNewFile for TempFile {
    type Target = TempFile;
    type Error = async_tempfile::Error;

    async fn new() -> Result<Self::Target, Self::Error> {
        TempFile::new().await
    }
}

impl FilePath for TempFile {
    fn file_path(&self) -> &PathBuf {
        self.file_path()
    }
}

/// A type alias for a [`SharedFile`] wrapping a [`TempFile`].
pub type SharedTemporaryFile = SharedFile<TempFile>;

/// A type alias for a [`SharedFileReader`] wrapping a [`TempFile`].
pub type SharedTemporaryFileReader = SharedFileReader<TempFile>;

/// A type alias for a [`SharedFileWriter`] wrapping a [`TempFile`].
pub type SharedTemporaryFileWriter = SharedFileWriter<TempFile>;

impl SharedTemporaryFile {
    /// Wraps a new instance of this type around an existing file. This is a convencience
    /// wrapper around [`TempFile::from_existing`] and [`SharedFile::from`].
    ///
    /// If `ownership` is set to [`Ownership::Borrowed`], this method does not take ownership of
    /// the file, i.e. the file will not be deleted when the instance is dropped.
    ///
    /// ## Arguments
    ///
    /// * `path` - The path of the file to wrap.
    /// * `ownership` - The ownership of the file.
    pub async fn from_existing(
        path: PathBuf,
        ownership: Ownership,
    ) -> Result<SharedFile<TempFile>, async_tempfile::Error> {
        let file = TempFile::from_existing(path, ownership).await?;
        Ok(Self::from(file))
    }
}
