//! This test will slowly write a file to disk while simultaneously
//! reading it from a different thread.
//!
//! Unlike `parallel_write_read.rs` this test reuses an existing file much
//! larger than the actual data being written. This is to ensure that
//! any spawned reader will not erroneously try to over-read the amount of
//! bytes it should read from the file.

use async_tempfile::{Ownership, TempFile};
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::sleep;

use shared_files::{FileSize, SharedFile, SharedTemporaryFile, SharedTemporaryFileReader};

/// The number of u16 values to prefill the file with.
const NUM_PREFILL_VALUES_U16: usize = 65_536;

/// The number of u16 values to write.
const NUM_VALUES_U16: usize = 3_724;

/// The number of bytes occupied by the written values.
const NUM_BYTES: usize = NUM_VALUES_U16 * std::mem::size_of::<u16>();

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_exact() {
    let original = TempFile::new()
        .await
        .expect("failed to create temporary file");

    // Pre-fill the original file and keep it around so it's deleted after the test.
    let original = prefill_file(SharedFile::from(original)).await;

    // Create a new wrapper around the original.
    let file =
        SharedTemporaryFile::from_existing(original.file_path().clone(), Ownership::Borrowed)
            .await
            .expect("failed to wrap the exiting file");

    // Spawn the readers first to ensure we can then move the writer.
    let reader_a = file.reader().await.expect("failed to create reader");
    let reader_b = reader_a.fork().await.expect("failed to create reader");

    // The file is indeed empty.
    assert!(matches!(reader_a.file_size(), FileSize::AtLeast(0)));
    assert!(matches!(reader_b.file_size(), FileSize::AtLeast(0)));

    // Attempt to read the file (nothing was written yet).
    let reader_future = tokio::spawn(parallel_read(reader_a));

    // Spawn the writer, moving the original file.
    let writer_future = tokio::spawn(parallel_write(file));

    // Wait for reader and writer to finish.
    let (writer_result, reader_result) = tokio::join!(writer_future, reader_future);
    assert!(writer_result.is_ok());
    assert!(reader_result.is_ok());

    // Ensure the first reader got the correct results.
    let result = reader_result.expect("reader failed");
    validate_result(result);

    // The file is not empty anymore.
    assert!(matches!(reader_b.file_size(), FileSize::Exactly(NUM_BYTES)));

    // Read from the written file.
    let result = parallel_read(reader_b).await;
    validate_result(result);
}

/// Ensures the result vector contains the correct sequence of values.
fn validate_result(read: Vec<u8>) {
    assert_eq!(read.len(), NUM_BYTES);
    read.chunks_exact(2)
        .map(|a| u16::from_ne_bytes([a[0], a[1]]))
        .enumerate()
        .for_each(|(i, value)| assert_eq!(value, i as u16));
}

/// Prefills the file.
async fn prefill_file(file: SharedTemporaryFile) -> SharedTemporaryFile {
    // Ensure the writer is dropped such that we don't mark the file as completed.
    {
        let mut writer = file.writer().await.expect("failed to create writer");
        for _ in 0..NUM_PREFILL_VALUES_U16 {
            writer.write_u16_le(0_u16).await.expect("failed to write");
        }

        // Ensure data is flushed to disk.
        writer.sync_all().await.expect("failed to sync");
    }

    file
}

/// Writes with arbitrary delays.
async fn parallel_write(file: SharedTemporaryFile) {
    let mut writer = file.writer().await.expect("failed to create writer");

    for i in 0..NUM_VALUES_U16 {
        writer
            .write_u16_le(i as u16)
            .await
            .expect("failed to write");

        if i % 100 == 0 {
            let t = thread_rng().gen_range(1..1000);
            sleep(Duration::from_micros(t)).await;

            writer.sync_data().await.expect("failed to sync data");
        }
    }

    writer.complete().await.expect("failed to complete write");
}

/// Reads while the writer is still active.
async fn parallel_read(mut reader: SharedTemporaryFileReader) -> Vec<u8> {
    let mut results = Vec::default();
    let mut buf = [0u8; 1024];
    loop {
        let read = reader
            .read(&mut buf)
            .await
            .expect("failed to read from file");
        results.extend_from_slice(&buf[..read]);
        if read == 0 {
            break;
        }
    }

    results
}
