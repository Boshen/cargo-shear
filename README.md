# Cargo Shear ✂️ 🐑

Detect and remove unused dependencies from `Cargo.toml` in Rust projects.

Does not work with transitive dependencies from macros (need to manually ignore).

## Installation

```bash
cargo binstall cargo-shear
# OR
cargo install cargo-shear
```

## Usage

```bash
cargo shear --fix
```

## CI

```yaml
- name: Install cargo-binstall
  uses: cargo-bins/cargo-binstall@main

- name: Install cargo-shear
  run: cargo binstall --no-confirm cargo-shear

- run: cargo shear
```

## Exit Code (for CI)

The exit code gives an indication whether unused dependencies have been found:

* 0 if found no unused dependencies,
* 1 if it found at least one unused dependency,
* 2 if there was an error during processing (in which case there's no indication whether any unused dependency was found or not).

## Ignore

Add to package's `Cargo.toml`

```toml
[package.metadata.cargo-shear]
ignored = ["crate"]
```

## Technique

1. use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. iterate through all package targets to locate all Rust files
3. use `syn` to parse these Rust files and extract imports
4. identify the difference between the imports and the package dependencies

## Prior Arts

* https://github.com/est31/cargo-udeps
* https://github.com/bnjbvr/cargo-machete

## Trophy Cases

* -7 lines from [oxc](https://github.com/oxc-project/oxc/pull/2729)
* -59 lines from [rspack](https://github.com/web-infra-dev/rspack/pull/5954)
* -39 lines from [rolldown](https://github.com/rolldown/rolldown/pull/593)
* -12 lines [ast-grep](https://github.com/ast-grep/ast-grep) [commit1](https://github.com/ast-grep/ast-grep/commit/c4ef252a71b05193f2ced327666f61836ad515c3) [commit2](https://github.com/ast-grep/ast-grep/commit/43edbc131e68173468e9aa302cab9b45263b1f76)
* -66 lines [biome](https://github.com/biomejs/biome/pull/2153)

## TODO

- [ ] make the reporting more granular for `[dependencies]`, `[dev-dependencies]` and `[build-dependencies]`
- [ ] add tests
- [ ] print things nicely

