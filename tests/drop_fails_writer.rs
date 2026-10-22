//! Regression test: dropping a writer without completing it must fail the write.
//!
//! A writer that is dropped before `complete`, `complete_no_sync` or `shutdown`
//! was abandoned, e.g. because the producing task was cancelled. Readers must
//! observe an error rather than a truncated file reported as complete, and
//! dropping a writer with unsynced bytes must not panic.

use std::io::ErrorKind;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

use shared_files::SharedTemporaryFile;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_writer_fails_parked_reader() {
    let file = SharedTemporaryFile::new_async()
        .await
        .expect("failed to create file");

    let reader = file.reader().await.expect("failed to create reader");
    let mut writer = file.writer().await.expect("failed to create writer");

    // Commit some data, then let the reader drain it and park waiting for more.
    writer
        .write_all(&[1, 2, 3, 4])
        .await
        .expect("failed to write");
    writer.sync_data().await.expect("failed to sync");

    let reader_task = tokio::spawn(async move {
        let mut reader = reader;
        let mut out = Vec::new();
        let mut buf = [0u8; 8];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => return Ok(out),
                Ok(read) => out.extend_from_slice(&buf[..read]),
                Err(e) => return Err((out, e.kind())),
            }
        }
    });

    // Let the reader consume the committed bytes and block.
    sleep(Duration::from_millis(50)).await;

    // Abandon the writer without completing it.
    drop(writer);

    let result = timeout(Duration::from_secs(5), reader_task)
        .await
        .expect("reader timed out waiting for the writer to be dropped")
        .expect("reader task panicked");

    let (out, kind) = result.expect_err("reader should observe the failed write");
    assert_eq!(out, [1, 2, 3, 4]);
    assert_eq!(kind, ErrorKind::BrokenPipe);
}

#[tokio::test]
async fn dropping_writer_with_unsynced_bytes_does_not_panic() {
    let file = SharedTemporaryFile::new_async()
        .await
        .expect("failed to create file");

    let mut writer = file.writer().await.expect("failed to create writer");
    writer
        .write_all(&[1, 2, 3, 4])
        .await
        .expect("failed to write");

    // No sync: the written bytes were never committed.
    drop(writer);

    let mut reader = file.reader().await.expect("failed to create reader");
    let mut out = Vec::new();
    let error = reader
        .read_to_end(&mut out)
        .await
        .expect_err("reader should observe the failed write");
    assert_eq!(error.kind(), ErrorKind::BrokenPipe);
}

#[tokio::test]
async fn dropping_completed_writer_keeps_file_complete() {
    let file = SharedTemporaryFile::new_async()
        .await
        .expect("failed to create file");

    let mut writer = file.writer().await.expect("failed to create writer");
    writer
        .write_all(&[1, 2, 3, 4])
        .await
        .expect("failed to write");
    writer.complete().await.expect("failed to complete write");

    let mut reader = file.reader().await.expect("failed to create reader");
    let mut out = Vec::new();
    reader.read_to_end(&mut out).await.expect("failed to read");
    assert_eq!(out, [1, 2, 3, 4]);
}
