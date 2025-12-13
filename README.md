# Cargo Shear ✂️ 🐑

Detect and fix issues in Rust projects:

- **Unused dependencies** in `Cargo.toml`
- **Misplaced dependencies** (dev/build dependencies in wrong sections)
- **Unlinked source files** (Rust files not reachable from any module tree)

## Installation

```bash
# Install from pre-built binaries.
cargo binstall cargo-shear

# Build from source.
cargo install cargo-shear

# Install from brew.
brew install cargo-shear
```

## Usage

Check for issues without making changes:

```bash
cargo shear
```

Automatically fix unused dependencies:

```bash
cargo shear --fix
```

Generate machine-readable JSON output:

```bash
cargo shear --format=json
```

This is particularly useful for CI/CD pipelines and custom tooling that need to programmatically process the results.

## Limitations

> [!IMPORTANT]
> `cargo shear` cannot detect "hidden" imports from macro expansions without the `--expand` flag (nightly only).
> This is because `cargo shear` uses rust-analyzer's parser to parse files and does not expand macros by default.

To expand macros:

```bash
cargo shear --expand --fix
```

The `--expand` flag uses `cargo expand`, which requires nightly and is significantly slower.

> [!IMPORTANT]
> Misplaced dependency detection only works for integration tests, benchmarks, and examples.
> Unit tests dependencies within `#[cfg(test)]` cannot be detected as misplaced.

## Configuration

### Ignore false positives

False positives can be ignored by adding them to the package's `Cargo.toml`:

```toml
[package.metadata.cargo-shear]
ignored = ["crate-name"]
```

### Ignore unlinked files

Unlinked files can be ignored using glob patterns:

```toml
[package.metadata.cargo-shear]
ignored-paths = ["src/proto/*.rs", "examples/old/*"]
```

Both options work in workspace `Cargo.toml` as well:

```toml
[workspace.metadata.cargo-shear]
ignored = ["crate-name"]
ignored-paths = ["*/proto/*.rs"]
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

### JSON Output for CI Integration

For CI systems that require structured output, use the `--format=json` flag:

```yaml
- name: Check for unused dependencies
  run: cargo shear --format=json > shear-results.json
```

The JSON output includes:
- **summary**: Counts of errors, warnings, and fixes
- **findings**: Detailed information about each issue including:
  - `code`: The diagnostic code (e.g., `shear/unused_dependency`)
  - `severity`: Error or warning level
  - `message`: Human-readable description
  - `file`: Path to the file with the issue
  - `location`: Byte offset and length within the file
  - `help`: Suggested fix
  - `fix`: Structured fix information

## Exit Code (for CI)

| Exit Code | Without `--fix` | With `--fix` |
|-----------|----------------|--------------|
| 0 | No issues found | No issues found, no changes made |
| 1 | Issues found | Issues found and fixed |
| 2 | Error during processing | Error during processing |

**GitHub Actions Example:**

```yaml
- name: cargo-shear
  shell: bash
  run: |
    if ! cargo shear --fix; then
      cargo check
    fi
```

## Technique

1. Use the `cargo_metadata` crate to list all dependencies specified in `[workspace.dependencies]` and `[dependencies]`
2. Iterate through all package targets (`lib`, `bin`, `example`, `test` and `bench`) to locate all Rust files
3. Use rust-analyzer's parser (`ra_ap_syntax`) to parse these Rust files and extract imports
   - Alternatively, use the `--expand` option with `cargo expand` to first expand macros and then parse the expanded code (though this is significantly slower)
4. Find the difference between the imports and the package dependencies

## Prior Art

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
* -86 lines [reqsign](https://github.com/Xuanwo/reqsign/pull/481)
* -184 lines from [turbopack](https://github.com/vercel/next.js/pull/80121)
* -625 lines from [openai/codex](https://github.com/openai/codex/pull/3338)

## [Sponsored By](https://github.com/sponsors/Boshen)

<p align="center">
  <a href="https://github.com/sponsors/Boshen">
    <img src="https://raw.githubusercontent.com/Boshen/sponsors/main/sponsors.svg" alt="My sponsors" />
  </a>
</p>
