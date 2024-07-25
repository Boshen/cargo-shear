# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.1](https://github.com/Boshen/cargo-shear/compare/v1.1.0...v1.1.1) - 2024-07-25

### Other
- *(deps)* update dependency rust to v1.80.0 ([#69](https://github.com/Boshen/cargo-shear/pull/69))

## [1.1.0](https://github.com/Boshen/cargo-shear/compare/v1.0.1...v1.1.0) - 2024-07-10

### Added
- inherit package level ignore from workspace level ignore ([#64](https://github.com/Boshen/cargo-shear/pull/64))

## [1.0.1](https://github.com/Boshen/cargo-shear/compare/v1.0.0...v1.0.1) - 2024-07-07

### Other
- macos-12

## [1.0.0](https://github.com/Boshen/cargo-shear/compare/v1.0.0...v1.0.0) - 2024-07-05

Release v1.0.0.

Consider `cargo-shear` as stable after using for a few months so we pin version in CI and introduce breaking changes in the future.

## [0.0.26](https://github.com/Boshen/cargo-shear/compare/v0.0.25...v0.0.26) - 2024-05-29

### Added
- exit code is 0 when performing fix ([#52](https://github.com/Boshen/cargo-shear/pull/52))

## [0.0.25](https://github.com/Boshen/cargo-shear/compare/v0.0.24...v0.0.25) - 2024-05-02

### Other
- *(deps)* update dependency rust to v1.78.0 ([#40](https://github.com/Boshen/cargo-shear/pull/40))
- *(renovate)* add rust-toolchain
- *(deps)* update rust crate cargo-util-schemas to 0.3.0 ([#39](https://github.com/Boshen/cargo-shear/pull/39))
- *(deps)* update rust crates ([#38](https://github.com/Boshen/cargo-shear/pull/38))
- *(deps)* update rust crate bpaf to 0.9.12 ([#37](https://github.com/Boshen/cargo-shear/pull/37))
- *(deps)* update rust crate cargo_toml to 0.20.2 ([#36](https://github.com/Boshen/cargo-shear/pull/36))
- *(deps)* update rust crate cargo_toml to 0.20.1 ([#35](https://github.com/Boshen/cargo-shear/pull/35))
- *(deps)* update rust crates ([#34](https://github.com/Boshen/cargo-shear/pull/34))
- *(deps)* update rust crate toml_edit to 0.22.11 ([#33](https://github.com/Boshen/cargo-shear/pull/33))
- *(deps)* update rust crate toml_edit to 0.22.10 ([#32](https://github.com/Boshen/cargo-shear/pull/32))
- *(deps)* update rust crate serde_json to 1.0.116 ([#31](https://github.com/Boshen/cargo-shear/pull/31))
- *(deps)* update rust crate anyhow to 1.0.82 ([#30](https://github.com/Boshen/cargo-shear/pull/30))
- mention `[workspace.metadata.cargo-shear]`

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
