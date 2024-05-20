# Changelog

All notable changes to this project will be documented in this file.
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2024-05-20

[0.2.0]: https://github.com/sunsided/shared-files-rs/releases/tag/v0.2.0

### Added

- Added the `exact_size` and `minimum_size` methods to the `FileSize` type.

## [0.1.0] - 2023-12-06

### Internal

- Upgraded to `async-tempfile` version `0.5.0`.
- The dependency on `thiserror` was removed.

## [0.0.8] - 2023-06-24

### Fixed

- A missing initialization hint in `0.0.7` left the bug unfixed. This is now resolved.

## [0.0.7] - 2023-06-24

### Fixed

- Fixed an issue where the read buffer would be advanced into uninitialized space.

## [0.0.6] - 2023-06-16

### Added

- Added support for [async-tempfile]'s `uuid` crate feature via the `SharedTemporaryFile::new_with_uuid` method.

## [0.0.5] - 2023-06-16

### Added

- Added a `file_path` method directly for `SharedTemporaryFile` so that the trait does
  not need to be implemented.

## [0.0.4] - 2023-06-14

### Internal

- Ensure documentation can be built with `cfg(doc)`. This should resolve
  [E0658](https://doc.rust-lang.org/error_codes/E0658.html) on docs.rs.

## [0.0.3] - 2023-06-14

### Added

- Added `prelude` module for simplified type imports.

## [0.0.2] - 2023-06-14

### Added

- Added the `async-tempfile` feature to enable the `SharedTempFile` type via the
  [async-tempfile] crate.
- Added the generic `SharedFile<T>` type for use with arbitrary `AsyncWrite + AsyncRead` types.

### Fixed

- Fixed an issue where re-using an existing file buffer would allow readers to
  eagerly read "uninitialized" data.

## [0.0.1] - 2023-06-13

### Internal

- 🎉 Initial release.

[0.1.0]: https://github.com/sunsided/shared-files-rs/releases/tag/0.1.0

[0.0.8]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.8

[0.0.7]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.7

[0.0.6]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.6

[0.0.5]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.5

[0.0.4]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.4

[0.0.3]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.3

[0.0.2]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.2

[0.0.1]: https://github.com/sunsided/shared-files-rs/releases/tag/0.0.1

[async-tempfile]: https://crates.io/crates/async-tempfile
