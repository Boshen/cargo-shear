# Cargo Shear

Detect and remove unused dependencies from `Cargo.toml` in Rust projects.

## Usage

```bash
cargo install cargo-shear
cargo shear --fix
```


## Technique

1. use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. iterate through all package targets to locate all Rust files
3. use `syn` to parse these Rust files and extract imports
4. identify the difference between the imports and the package dependencies

## TODO

- [ ] make the reporting more granular for `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]`
- [ ] `--fix` the root Cargot.toml
- [ ] add tests
- [ ] exit codes
- [ ] error recovery
- [ ] print things more nicely

## Prior Arts

* https://github.com/est31/cargo-udeps
* https://github.com/bnjbvr/cargo-machete

## Trophy Cases

* -7 lines from [oxc](https://github.com/oxc-project/oxc/pull/2729)
* -59 lines from [rspack](https://github.com/web-infra-dev/rspack/pull/5954)
* -39 lines from [rolldown](https://github.com/rolldown/rolldown/pull/593)
