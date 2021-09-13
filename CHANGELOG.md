# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added new if expression syntax `{{@if !{{VARIABLE}}}}` for templates ([#67](https://github.com/Shemnei/punktf/pull/67))
- Added informational message when running `deploy` with the `dry-run` flag ([#61](https://github.com/Shemnei/punktf/pull/61))
- Added better error messages when profile parsing fails ([#62](https://github.com/Shemnei/punktf/pull/62))
- Added better error messages when punktf source directory or subdirectories are missing ([#65](https://github.com/Shemnei/punktf/pull/65))
- Yaml/Json profiles are now separate features and can be added/removed via the `Cargo.toml` of `punktf` ([#69](https://github.com/Shemnei/punktf/pull/69)). For available options see punktf-lib's [Cargo.toml](https://github.com/Shemnei/punktf/blob/main/crates/punktf-lib/Cargo.toml).
- Added `ContentTransformer` which can modify the contents of a dotfile just before deploying it ([#59](https://github.com/Shemnei/punktf/pull/59))
- Added `LineTerminator` content transformer which can transform between unix (`\n`) and windows (`\r\n`) style line terminators ([#59](https://github.com/Shemnei/punktf/pull/59))

### Changed

- Changed release profile to decrease compile times and final binary size ([#60](https://github.com/Shemnei/punktf/pull/60))

### Removed

- Removed current directory as fallback for the `punktf` source directory when neither `-s/--source` or `PUNKTF_SOURCE` were supplied. This mainly caused confusion and was a undocumented feature.

### Fixed

- Fixed a bug where an empty new line would be emitted if a line starting `comment`, `print`, `if` or `escaped` block resolved to an empty string ([#70](https://github.com/Shemnei/punktf/issues/70))

## [1.0.1] - 2021-09-03

### Fixed

- Fixed bug where `punktf` would always panic/crash if environment variable `PUNKTF_TARGET` was not set ([#57](https://github.com/Shemnei/punktf/issues/57))

## [1.0.0] - 2021-08-20

Initial release

## [1.0.0-alpha] - 2021-08-08

Initial alpha release

[Unreleased]: https://github.com/Shemnei/punktf/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/Shemnei/punktf/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/Shemnei/punktf/compare/v1.0.0-alpha...v1.0.0
[1.0.0-alpha]: https://github.com/Shemnei/punktf/releases/tag/v1.0.0-alpha
