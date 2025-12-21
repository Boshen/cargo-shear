# cargo-shear

Detect and fix unused dependencies, misplaced dependencies, and unlinked source files in Rust projects.

## Development

Run `just ready` after making code changes to ensure code quality and all tests pass.

### Frequent Commands

- `just ready` - Run full quality checks (typos, fmt, check, lint, test)
- `just fmt` - Format code with rustfmt and taplo
- `just lint` - Run clippy
- `just snapshots` - Accept all snapshot test changes
- `cargo test` - Run all tests
- `cargo check` - Quick compile check

## Key Modules

- `src/lib.rs` - Core library functionality
- `src/package_analyzer.rs` - Analyzes package dependencies
- `src/source_parser.rs` - Parses Rust source files for imports
- `src/cargo_toml_editor.rs` - Edits Cargo.toml files
- `src/package_processor.rs` - Processes packages in workspaces

## Testing

- Integration tests: `tests/integration_tests.rs`
- Test fixtures: `tests/fixtures/`
- Snapshot tests use `cargo-insta`
