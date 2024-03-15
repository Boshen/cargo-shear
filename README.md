# Cargo Sheer

Detect and remove unused dependencies from `Cargo.toml` in Rust projects.

## Technique

1. use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. iterate through all package targets to locate all Rust files
3. use `syn` to parse these Rust files and extract imports
4. identify the difference between the imports and the package dependencies

## False positives (to be solved):

* macros
  * how can imports be collected from macros? e.g. in `println!({}, foo::bar)`, `foo` is a macro token instead of an identifier
  * can we run some other command and get the macro expanded source to parse?
  * is there an API for getting imports instead of parsing?

## TODO

- [ ] make the reporting more granular for `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]`
- [ ] `--fix`
- [ ] add tests
- [ ] exit codes
- [ ] error recovery
- [ ] print things more nicely

### False positives to be fixed

- [ ] package renames
- [ ] uppercase package name, e.g. `Inflector`
- [ ] pathname in type arguments

```rust
    pub type ModuleIdToFileName = std::sync::Arc<
  dashmap::DashMap<
    rspack_core::ModuleIdentifier,
    (String, AssetInfo),
    std::hash::BuildHasherDefault<rspack_identifier::IdentifierHasher>,
  >,
>;
```


## Prior Arts

* https://github.com/est31/cargo-udeps
* https://github.com/bnjbvr/cargo-machete

## Trophy Cases
