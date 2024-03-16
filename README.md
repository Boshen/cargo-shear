# Cargo Shear

Detect and remove unused dependencies from `Cargo.toml` in Rust projects.

## Usage

```bash
cargo install cargo-shear
cargo shear
```


## Technique

1. use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. iterate through all package targets to locate all Rust files
3. use `syn` to parse these Rust files and extract imports
4. identify the difference between the imports and the package dependencies

## TODO

- [ ] make the reporting more granular for `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]`
- [ ] `--fix`
- [ ] add tests
- [ ] exit codes
- [ ] error recovery
- [ ] print things more nicely

## Prior Arts

* https://github.com/est31/cargo-udeps
* https://github.com/bnjbvr/cargo-machete

## Trophy Cases
