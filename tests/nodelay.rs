//! This test will slowly write a file to disk while simultaneously
//! reading it from a different thread.
//!
//! Same as `parallel_write_read.rs` but without the artifical delays.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared_files::{FileSize, SharedTemporaryFile, SharedTemporaryFileReader};

/// The number of u16 values to write.
const NUM_VALUES_U16: usize = 1_048_576;

/// The number of bytes occupied by the written values.
const NUM_BYTES: usize = NUM_VALUES_U16 * std::mem::size_of::<u16>();

#[tokio::test(flavor = "multi_thread")]
async fn nodelay() {
    let file = SharedTemporaryFile::new_async()
        .await
        .expect("failed to create file");

    // Spawn the readers first to ensure we can then move the writer.
    let reader_a = file.reader().await.expect("failed to create reader");

    // The file is indeed empty.
    assert!(matches!(reader_a.file_size(), FileSize::AtLeast(0)));

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
}

/// Ensures the result vector contains the correct sequence of values.
fn validate_result(read: Vec<u8>) {
    assert_eq!(read.len(), NUM_BYTES);
    read.chunks_exact(2)
        .map(|a| u16::from_ne_bytes([a[0], a[1]]))
        .enumerate()
        .for_each(|(i, value)| assert_eq!(value, i as u16));
}

/// Writes with arbitrary delays.
async fn parallel_write(file: SharedTemporaryFile) {
    let mut writer = file.writer().await.expect("failed to create writer");

    for i in 0..NUM_VALUES_U16 {
        writer
            .write_u16_le(i as u16)
            .await
            .expect("failed to write");

        // Every so often, sync to disk.
        if i % 4096 == 0 {
            writer.flush().await.expect("failed to sync data");
        }
    }

    writer.complete().await.expect("failed to complete write");
}

/// Reads the file (while the writer is still active).
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
