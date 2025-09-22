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
   - `src/dependency_analyzer.rs` - Analyzes dependencies
   - `src/import_collector.rs` - Collects imports from Rust code
   - `src/cargo_toml_editor.rs` - Edits Cargo.toml files
   - `src/package_processor.rs` - Processes packages in workspaces

5. **Development Workflow**
   - Make changes
   - Run `just ready` to check everything
   - Ensure all checks pass before considering work complete