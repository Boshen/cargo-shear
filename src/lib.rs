//! # cargo-shear
//!
//! A tool for detecting and removing unused dependencies from Rust projects.
//!
//! ## Overview
//!
//! `cargo-shear` analyzes your Rust codebase to identify dependencies that are declared
//! in `Cargo.toml` but never actually used in the code. It can automatically remove
//! these unused dependencies with the `--fix` flag.
//!
//! ## Architecture
//!
//! The codebase is organized into several focused modules:
//!
//! - `cargo_toml_editor` - Handles modifications to Cargo.toml files
//! - `dependency_analyzer` - Analyzes code to find used dependencies
//! - `package_processor` - Processes packages and detects unused dependencies
//! - `import_collector` - Parses Rust source to collect import statements
//! - `error` - Custom error types with detailed context
//!
//! ## Usage
//!
//! ```no_run
//! use cargo_shear::{CargoShear, CargoShearOptions};
//!
//! let options = CargoShearOptions::new_for_test(
//!     std::path::PathBuf::from("."),
//!     false, // fix
//! );
//! let exit_code = CargoShear::new(std::io::stdout(), options).run();
//! ```

mod cargo_toml_editor;
mod dependency_analyzer;
mod import_collector;
mod package_processor;
#[cfg(test)]
mod tests;

use std::{env, io::Write, path::PathBuf, process::ExitCode};

use anyhow::Result;
use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, MetadataCommand};
use cargo_toml::Manifest;
use rustc_hash::FxHashSet;

use crate::{cargo_toml_editor::CargoTomlEditor, package_processor::PackageProcessor};

const VERSION: &str = match option_env!("SHEAR_VERSION") {
    Some(v) => v,
    None => "dev",
};

/// Command-line options for cargo-shear.
///
/// This struct is parsed from command-line arguments using `bpaf`.
/// The "batteries" feature strips the binary name using `bpaf::cargo_helper`.
///
/// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options("shear"), version(VERSION))]
pub struct CargoShearOptions {
    /// Remove unused dependencies.
    ///
    /// When set, cargo-shear will automatically remove detected unused
    /// dependencies from Cargo.toml files.
    #[bpaf(long)]
    fix: bool,

    /// Uses `cargo expand` to expand macros, which requires nightly and is significantly slower.
    ///
    /// This option provides more accurate detection by expanding proc macros
    /// and attribute macros, but requires a nightly Rust toolchain.
    #[bpaf(long)]
    expand: bool,

    /// Assert that `Cargo.lock` will remain unchanged.
    locked: bool,

    /// Run without accessing the network
    offline: bool,

    /// Equivalent to specifying both --locked and --offline
    frozen: bool,

    /// Package(s) to check.
    ///
    /// If not specified, all packages in the workspace are checked.
    /// Can be specified multiple times to check specific packages.
    #[bpaf(long, short, argument("SPEC"))]
    package: Vec<String>,

    /// Exclude packages from the check.
    ///
    /// Can be specified multiple times to exclude multiple packages.
    exclude: Vec<String>,

    /// Path to the project directory.
    ///
    /// Defaults to the current directory if not specified.
    #[bpaf(positional("PATH"), fallback_with(default_path))]
    path: PathBuf,
}

impl CargoShearOptions {
    /// Create a new `CargoShearOptions` for testing purposes
    #[must_use]
    pub const fn new_for_test(path: PathBuf, fix: bool) -> Self {
        Self {
            fix,
            expand: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec![],
            path,
        }
    }
}

pub(crate) fn default_path() -> Result<PathBuf> {
    Ok(env::current_dir()?)
}

/// The main struct that orchestrates the dependency analysis and removal process.
///
/// `CargoShear` coordinates the analysis of a Rust project to find unused dependencies
/// and optionally removes them from Cargo.toml files.
pub struct CargoShear<W> {
    /// Writer for output
    writer: W,

    /// Configuration options for the analysis
    options: CargoShearOptions,

    /// Counter for total unused dependencies found
    unused_dependencies: usize,

    /// Counter for dependencies that were fixed (removed)
    fixed_dependencies: usize,
}

impl<W: Write> CargoShear<W> {
    /// Create a new `CargoShear` instance with the given options.
    ///
    /// # Arguments
    ///
    /// * `writer` - Output writer
    /// * `options` - Configuration options for the analysis
    ///
    /// # Example
    ///
    /// ```
    /// use cargo_shear::{CargoShear, CargoShearOptions};
    /// use std::path::PathBuf;
    ///
    /// let options = CargoShearOptions::new_for_test(PathBuf::from("."), false);
    /// let shear = CargoShear::new(std::io::stdout(), options);
    /// ```
    #[must_use]
    pub const fn new(writer: W, options: CargoShearOptions) -> Self {
        Self { writer, options, unused_dependencies: 0, fixed_dependencies: 0 }
    }

    /// Run the dependency analysis and optionally fix unused dependencies.
    ///
    /// This method performs the complete analysis workflow:
    /// 1. Analyzes all packages in the workspace
    /// 2. Detects unused dependencies
    /// 3. Optionally removes them if `--fix` is enabled
    /// 4. Reports results to the writer
    ///
    /// # Returns
    ///
    /// Returns an `ExitCode` indicating success or failure:
    /// - `0` if no issues were found or all issues were fixed
    /// - `1` if unused dependencies were found (without `--fix`)
    /// - `2` if an error occurred
    #[must_use]
    pub fn run(mut self) -> ExitCode {
        let _ = writeln!(self.writer, "Analyzing {}", self.options.path.to_string_lossy());
        let _ = writeln!(self.writer);

        match self.shear() {
            Ok(()) => {
                let has_fixed = self.fixed_dependencies > 0;

                if has_fixed {
                    let _ = writeln!(
                        self.writer,
                        "Fixed {} {}.\n",
                        self.fixed_dependencies,
                        if self.fixed_dependencies == 1 { "dependency" } else { "dependencies" }
                    );
                }

                let has_deps = (self.unused_dependencies - self.fixed_dependencies) > 0;

                if has_deps {
                    let _ = writeln!(
                        self.writer,
                        "\n\
                        cargo-shear may have detected unused dependencies incorrectly due to its limitations.\n\
                        They can be ignored by adding the crate name to the package's Cargo.toml:\n\n\
                        [package.metadata.cargo-shear]\n\
                        ignored = [\"crate-name\"]\n\n\
                        or in the workspace Cargo.toml:\n\n\
                        [workspace.metadata.cargo-shear]\n\
                        ignored = [\"crate-name\"]\n"
                    );
                } else {
                    let _ = writeln!(self.writer, "No unused dependencies!");
                }

                ExitCode::from(u8::from(if self.options.fix { has_fixed } else { has_deps }))
            }
            Err(err) => {
                let _ = writeln!(self.writer, "{err:?}");
                let _ = writeln!(
                    self.writer,
                    "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
                );
                ExitCode::from(2)
            }
        }
    }

    fn shear(&mut self) -> Result<()> {
        let mut extra_opts = Vec::new();
        if self.options.locked {
            extra_opts.push("--locked".to_owned());
        }
        if self.options.offline {
            extra_opts.push("--offline".to_owned());
        }
        if self.options.frozen {
            extra_opts.push("--frozen".to_owned());
        }

        let metadata = MetadataCommand::new()
            .features(CargoOpt::AllFeatures)
            .current_dir(&self.options.path)
            .other_options(extra_opts)
            .exec()
            .map_err(|e| anyhow::anyhow!("Metadata error: {e}"))?;

        let processor = PackageProcessor::new(self.options.expand);

        // Track all packages used across the workspace
        let mut workspace_used_pkgs = FxHashSet::default();

        for package in metadata.workspace_packages() {
            // Skip if package is in the exclude list
            if self.options.exclude.iter().any(|name| name == package.name.as_str()) {
                continue;
            }

            // Skip if specific packages are specified and this package is not in the list
            if !self.options.package.is_empty()
                && !self.options.package.iter().any(|name| name == package.name.as_str())
            {
                continue;
            }

            let manifest = Manifest::from_path(package.manifest_path.as_std_path())?;
            let result = processor.process_package(&metadata, package, &manifest)?;

            // Warn about redundant ignores
            for ignored_dep in &result.redundant_ignores {
                writeln!(
                    self.writer,
                    "warning: '{ignored_dep}' is redundant in [package.metadata.cargo-shear] for package '{}'.\n",
                    package.name
                )?;
            }

            if !result.unused_dependencies.is_empty() {
                let relative_path = PackageProcessor::get_relative_path(
                    package.manifest_path.as_std_path(),
                    metadata.workspace_root.as_std_path(),
                );

                writeln!(self.writer, "{} -- {}:", package.name, relative_path.display())?;
                for unused_dep in &result.unused_dependencies {
                    writeln!(self.writer, "  {unused_dep}")?;
                }
                writeln!(self.writer)?;

                self.unused_dependencies += result.unused_dependencies.len();

                if self.options.fix {
                    let fixed = CargoTomlEditor::remove_dependencies(
                        package.manifest_path.as_std_path(),
                        &result.unused_dependencies,
                    )?;
                    self.fixed_dependencies += fixed;
                }
            }

            workspace_used_pkgs.extend(result.used_packages);
        }

        // Process workspace dependencies
        let cargo_toml_path = metadata.workspace_root.as_std_path().join("Cargo.toml");
        let manifest = Manifest::from_path(&cargo_toml_path)?;

        let workspace_result =
            PackageProcessor::process_workspace(&manifest, &metadata, &workspace_used_pkgs);

        // Warn about redundant workspace ignores
        for ignored_dep in &workspace_result.redundant_ignores {
            writeln!(
                self.writer,
                "warning: '{ignored_dep}' is redundant in [workspace.metadata.cargo-shear].\n"
            )?;
        }

        if !workspace_result.unused_dependencies.is_empty() {
            let path = cargo_toml_path
                .strip_prefix(env::current_dir().unwrap_or_default())
                .unwrap_or(&cargo_toml_path)
                .to_string_lossy();

            writeln!(self.writer, "root -- {path}:")?;
            for unused_dep in &workspace_result.unused_dependencies {
                writeln!(self.writer, "  {unused_dep}")?;
            }
            writeln!(self.writer)?;

            self.unused_dependencies += workspace_result.unused_dependencies.len();

            if self.options.fix {
                let fixed = CargoTomlEditor::remove_dependencies(
                    &cargo_toml_path,
                    &workspace_result.unused_dependencies,
                )?;
                self.fixed_dependencies += fixed;
            }
        }

        Ok(())
    }
}
