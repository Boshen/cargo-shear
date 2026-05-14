# cargo-shear

Detect and fix unused dependencies, misplaced dependencies, and unlinked source files in Rust projects.

## Development workflow

After making code changes, run `just ready` — it runs typos, fmt, check, lint, and tests, and must pass before work is considered complete.

## Frequent commands

- `just ready` — full quality checks (typos, fmt, check, lint, test)
- `just fmt` — format with rustfmt and taplo
- `just lint` — clippy
- `just snapshots` — accept snapshot test changes
- `cargo test` — run all tests
- `cargo check` — quick compile check

## Code standards

- Follow existing Rust idioms and patterns in the codebase.
- All code must pass the strict lints configured in `Cargo.toml`.
- Prefer `ast-grep` for syntax-aware searches in Rust source.

## Key modules

- `src/lib.rs` — core library entry point
- `src/source_parser.rs` — parses Rust source for imports and file paths
- `src/package_analyzer.rs` — finds unused dependencies and unlinked files
- `src/package_processor.rs` — processes packages (unused, misplaced deps, etc.)
- `src/cargo_toml_editor.rs` — edits `Cargo.toml`
- `src/manifest.rs` — manifest parsing and types
- `src/context.rs` — workspace and package context types
- `src/diagnostics.rs` — diagnostic types and analysis results
- `src/output.rs`, `src/output/` — output formatting (miette, JSON, GitHub Actions)

## Testing

- Unit tests: `src/tests.rs`
- Integration tests: `tests/integration_tests.rs`
- Test fixtures: `tests/fixtures/`
- Snapshot tests use `cargo-insta` (`just snapshots` to accept changes)
