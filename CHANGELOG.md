# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Added

- Added the `async-tempfile` crate feature to enable the `SharedTempFile`
  type.
- Added the generic `SharedFile<T>` type for use with arbitrary `AsyncWrite + AsyncRead + Unpin` types.

## [0.0.1] - 2023-06-13

### Internal

- 🎉 Initial release.

[0.0.1]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.1
