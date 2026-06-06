//! Regression test: completing a writer via `shutdown` must wake parked readers.
//!
//! A reader that has consumed all currently committed bytes parks itself until
//! the writer commits more. `AsyncWriteExt::shutdown` (called e.g. by
//! `tokio::io::copy`) transitions the file to the completed state, but if it
//! fails to wake the parked readers they hang forever instead of observing EOF.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

use shared_files::SharedTemporaryFile;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_wakes_parked_reader() {
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
            let read = reader.read(&mut buf).await.expect("failed to read");
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read]);
        }
        out
    });

    // Let the reader consume the committed bytes and block.
    sleep(Duration::from_millis(50)).await;

    // Finalize the writer via the `AsyncWrite` shutdown path.
    writer.shutdown().await.expect("failed to shut down writer");

    let out = timeout(Duration::from_secs(5), reader_task)
        .await
        .expect("reader timed out waiting for shutdown")
        .expect("reader task panicked");

    assert_eq!(out, [1, 2, 3, 4]);
}
