# Project: cargo-shear

A Rust tool for detecting and removing unused dependencies from Cargo.toml files.

## Important Instructions

1. **After making any code changes**, always run:
   ```bash
   just ready
   ```
   This ensures code quality and all tests pass.

2. **Code Standards**
   - Follow existing Rust idioms and patterns in the codebase
   - Maintain the existing code style and formatting
   - Use `ast-grep` for syntax-aware searches when needed
   - All code must pass the strict linting rules defined in Cargo.toml

3. **Testing**
   - Run tests with `cargo test`
   - Test fixtures are in `tests/fixtures/`
   - Integration tests are in `src/tests.rs`

4. **Key Modules**
   - `src/lib.rs` - Core library functionality
   - `src/source_parser.rs` - Parses Rust source files to extract imports and file paths
   - `src/package_analyzer.rs` - Analyzes packages to find unused dependencies and unlinked files
   - `src/package_processor.rs` - Processes packages to identify issues (unused, misplaced deps, etc.)
   - `src/cargo_toml_editor.rs` - Edits Cargo.toml files
   - `src/manifest.rs` - Manifest parsing and types
   - `src/context.rs` - Context types for workspaces and packages
   - `src/diagnostics.rs` - Diagnostic types and analysis results
   - `src/output.rs` - Output formatting (miette, JSON, GitHub Actions)

5. **Development Workflow**
   - Make changes
   - Run `just ready` to check everything
   - Ensure all checks pass before considering work complete