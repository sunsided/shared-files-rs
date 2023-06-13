use crate::{
    AsyncNewFile, CompleteWritingError, FilePath, SharedFile, SharedFileReader, SharedFileType,
    SharedFileWriter,
};
use async_tempfile::TempFile;
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
pub type SharedTempFile = SharedFile<TempFile>;

/// A type alias for a [`SharedFileReader`] wrapping a [`TempFile`].
pub type SharedTempFileReader = SharedFileReader<TempFile>;

/// A type alias for a [`SharedFileWriter`] wrapping a [`TempFile`].
pub type SharedTempFileWriter = SharedFileWriter<TempFile>;
