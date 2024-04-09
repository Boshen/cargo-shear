# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.24](https://github.com/Boshen/cargo-shear/compare/v0.0.23...v0.0.24) - 2024-04-09

### Added
- handle package rename in workspace dependencies
- add ignore with [workspace.metadata.cargo-shear]

### Other
- space out printing

## [0.0.23](https://github.com/Boshen/cargo-shear/compare/v0.0.22...v0.0.23) - 2024-04-03

### Fixed
- collect import from all use declarations

### Other
- use [lints.clippy]

## [0.0.22](https://github.com/Boshen/cargo-shear/compare/v0.0.21...v0.0.22) - 2024-04-03

### Fixed
- rust v1.77.0 has a different package id representation

## [0.0.21](https://github.com/Boshen/cargo-shear/compare/v0.0.20...v0.0.21) - 2024-04-03

### Other
- fix github.ref read

## [0.0.20](https://github.com/Boshen/cargo-shear/compare/v0.0.19...v0.0.20) - 2024-04-03

### Added
- add --version

### Other
- simplify code around hashset union
- analyze packages in sequence, make debugging easier
- setup rust with moonrepo

## [0.0.19](https://github.com/Boshen/cargo-shear/compare/v0.0.18...v0.0.19) - 2024-04-02

### Fixed
- use `--all-features` to get all deps

### Other
- update README

## [0.0.18](https://github.com/Boshen/cargo-shear/compare/v0.0.17...v0.0.18) - 2024-04-02

### Added
- use cargo metadata module resolution to get module names instead of package names
- add `profile.release` to Cargo.toml

### Other
- small tweaks

## [0.0.17](https://github.com/Boshen/cargo-shear/compare/v0.0.16...v0.0.17) - 2024-04-01

### Fixed
- ignored packages by package name instead of normalized name

### Other
- fix broken ci
- make `shear_package` the more readable
- minor tweak
- add `--no-deps` to `cargo metadata`
- add `just ready`
- run shear on this repo

## [0.0.16](https://github.com/Boshen/cargo-shear/compare/v0.0.15...v0.0.16) - 2024-03-29

### Added
- better output messages

### Other
- update README about ignoring false positives

## [0.0.15](https://github.com/Boshen/cargo-shear/compare/v0.0.14...v0.0.15) - 2024-03-26

### Other
- fix release

## [0.0.14](https://github.com/Boshen/cargo-shear/compare/v0.0.13...v0.0.14) - 2024-03-26

### Other
- fix release-binaries

## [0.0.13](https://github.com/Boshen/cargo-shear/compare/v0.0.12...v0.0.13) - 2024-03-26

### Fixed
- binary release

### Other
- Rust v1.77.0

## [0.0.12](https://github.com/Boshen/cargo-shear/compare/v0.0.11...v0.0.12) - 2024-03-26

### Other
- add binary after release

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
