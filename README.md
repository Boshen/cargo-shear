# Cargo Shear ✂️ 🐑

Detect and remove unused dependencies from `Cargo.toml` in Rust projects.

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

## Ignore false positives

> [!IMPORTANT]
> `cargo shear` cannot detect dependency usages from macros because it uses `syn` to parse the files

False positives can be ignored by adding them to the package's `Cargo.toml`:

```toml
[package.metadata.cargo-shear]
ignored = ["crate"]
```

or in workspace's `Cargo.toml`:

```toml
[workspace.metadata.cargo-shear]
ignored = ["crate"]
```

Otherwise please report the issue as a bug.

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

## Technique

1. use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. iterate through all package targets (`lib`, `bin`, `example`, `test` and `bench`) to locate all Rust files
3. use `syn` to parse these Rust files and extract imports
4. find the difference between the imports and the package dependencies

## Prior Arts

* [est31/cargo-udeps](https://github.com/est31/cargo-udeps)
    * it collects dependency usage by compiling your project and find them from the `target/` directory
    * does not seem to work anymore with the latest versions of `cargo`
    * does not work with cargo workspaces
* [bnjbvr/cargo-machete](https://github.com/bnjbvr/cargo-machete)
    * it collects dependency usage by running regex patterns on source code
    * does not detect all usages of a dependency
    * does not remove unused dependencies from the workspace root
* cargo and clippy
    * There was intention to add similar features to cargo or clippy, but the progress is currently stagnant
    * See https://github.com/rust-lang/rust/issues/57274 and https://github.com/rust-lang/rust-clippy/issues/4341

## Trophy Cases

* -7 lines from [oxc](https://github.com/oxc-project/oxc/pull/2729)
* -59 lines from [rspack](https://github.com/web-infra-dev/rspack/pull/5954)
* -39 lines from [rolldown](https://github.com/rolldown/rolldown/pull/593)
* -12 lines [ast-grep](https://github.com/ast-grep/ast-grep) [commit1](https://github.com/ast-grep/ast-grep/commit/c4ef252a71b05193f2ced327666f61836ad515c3) [commit2](https://github.com/ast-grep/ast-grep/commit/43edbc131e68173468e9aa302cab9b45263b1f76)
* -66 lines [biome](https://github.com/biomejs/biome/pull/2153)
* -164 lines [astral-sh/uv](https://github.com/astral-sh/uv/pull/3527)

## [Sponsored By](https://github.com/sponsors/Boshen)

<p align="center">
  <a href="https://github.com/sponsors/Boshen">
    <img src="https://raw.githubusercontent.com/Boshen/sponsors/main/sponsors.svg" alt="My sponsors" />
  </a>
</p>
