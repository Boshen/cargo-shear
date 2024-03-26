# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.11](https://github.com/Boshen/cargo-shear/compare/v0.0.10...v0.0.11) - 2024-03-26

### Other
- add release-plz
- add typos
- add `cargo publish`

## v0.0.10 - 2024-03-25

### Fixed

* Return exit code 0 when there are no unused dependencies, 1 when there are unused dependencies.

## v0.0.9 - 2024-03-25

### Added

* Ignore crate by `[package.metadata.cargo-shear] ignored = ["crate"]`
