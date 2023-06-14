# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.4] - 2023-06-14

### Internal

- Ensure documentation can be built with `cfg(doc)`. This should resolve
  [E0658](https://doc.rust-lang.org/error_codes/E0658.html) on docs.rs. 

## [0.0.3] - 2023-06-14

### Added

- Added `prelude` module for simplified type imports.

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

[0.0.4]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.4
[0.0.3]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.3
[0.0.2]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.2
[0.0.1]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.1
