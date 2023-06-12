# Shared Files

✨ _Disk-Based Single-Writer, Multiple-Reader In-Process File Sharing_ ✨

---

Functionality for single-writer, multiple-reader file operations where multiple concurrent readers
need to read from a file that is currently being written by the same process. The intended use case is the parallel
processing of byte streams with minimum (process) memory requirements, e.g. in web services moving around large files.

Normally, reading a file while it is written results in the read stream ending prematurely as EOF; the purpose
of this crate is to prevent exactly that.

The functionality is currently based on the [async-tempfile](https://github.com/sunsided/async-tempfile-rs) crate.
A generic implementation is planned for the use of arbitrary `AsyncWrite` / `AsyncRead` backing.

## Example

See [`tests/parallel_write_read.rs`](tests/parallel_write_read.rs) for a usage example.
