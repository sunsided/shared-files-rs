# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.2] - 2023-06-14

### Added

- Added the `async-tempfile` crate feature to enable the `SharedTempFile`
  type.
- Added the generic `SharedFile<T>` type for use with arbitrary `AsyncWrite + AsyncRead` types.

### Fixed

- Fixed an issue where re-using an existing file buffer would allow readers to
  eagerly read "uninitialized" data.

## [0.0.1] - 2023-06-13

### Internal

- 🎉 Initial release.

[0.0.2]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.2
[0.0.1]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.1
