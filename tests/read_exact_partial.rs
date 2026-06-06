//! Regression test for partial reads that span a commit boundary.
//!
//! `read_exact` drives `poll_read` repeatedly against the same `ReadBuf`,
//! accumulating into its filled region across polls. If the reader reads fewer
//! bytes than requested on the first poll (because only part of the data has
//! been committed) and more data is committed afterwards, the second poll must
//! extend the already-filled region rather than overwrite its length. A reader
//! that updates the parent buffer with an absolute length instead of a relative
//! advance would drop the first chunk and surface a spurious `UnexpectedEof`.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};

use shared_files::SharedTemporaryFile;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_exact_spans_partial_commit() {
    let file = SharedTemporaryFile::new_async()
        .await
        .expect("failed to create file");

    let reader = file.reader().await.expect("failed to create reader");
    let mut writer = file.writer().await.expect("failed to create writer");

    // Commit the first half of the payload.
    writer
        .write_all(&[1, 2, 3, 4])
        .await
        .expect("failed to write first half");
    writer.sync_data().await.expect("failed to sync first half");

    // The reader asks for the whole payload; it will consume the committed four
    // bytes, then park waiting for the rest.
    let reader_task = tokio::spawn(async move {
        let mut reader = reader;
        let mut buf = [0u8; 8];
        reader
            .read_exact(&mut buf)
            .await
            .expect("read_exact must fill the whole buffer");
        buf
    });

    // Give the reader a chance to consume the first chunk and block.
    sleep(Duration::from_millis(50)).await;

    // Commit the remaining half.
    writer
        .write_all(&[5, 6, 7, 8])
        .await
        .expect("failed to write second half");
    writer
        .sync_data()
        .await
        .expect("failed to sync second half");

    let buf = timeout(Duration::from_secs(5), reader_task)
        .await
        .expect("reader timed out")
        .expect("reader task panicked");

    assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8]);
}
