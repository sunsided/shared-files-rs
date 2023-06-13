# Shared Files

✨ _Disk-Based Single-Writer, Multiple-Reader In-Process File Sharing_ ✨

---

Functionality for single-writer, multiple-reader file operations where multiple concurrent readers
need to read from a file that is currently being written by the same process. The intended use case is the parallel
processing of byte streams with minimum (process) memory requirements, e.g. in web services moving around large files.

Normally, reading a file while it is written results in the read stream ending prematurely as EOF; the purpose
of this crate is to prevent exactly that.

Any file type can be used as a backing as long as it implements the crate's `SharedFileType` trait, which in turn
requires `AsyncWrite`, `AsyncRead` and `Unpin`.

## Features

- `async-tempfile`: Enables the `SharedTempFile` type via the [async-tempfile](https://github.com/sunsided/async-tempfile-rs)
  crate. Since this is how this crate was initially meant to be used, this feature is enabled by default.

## Example

See [`tests/parallel_write_read.rs`](tests/parallel_write_read.rs) for a usage example.
The example requires the `async-tempfile` crate feature. To run it, use e.g.

```shell
cargo test parallel_write_read --features=async-tempfile
```
