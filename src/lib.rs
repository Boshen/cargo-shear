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

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use anyhow::Result;
use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package};
use cargo_toml::Manifest;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashSet;
use toml_edit::DocumentMut;

use crate::{
    cargo_toml_editor::CargoTomlEditor,
    package_processor::{PackageProcessResult, PackageProcessor, WorkspaceProcessResult},
};

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

    /// Counter for total misplaced dependencies found
    misplaced_dependencies: usize,

    /// Counter for dependencies that were fixed
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
        Self {
            writer,
            options,
            unused_dependencies: 0,
            misplaced_dependencies: 0,
            fixed_dependencies: 0,
        }
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

                let total_issues = self.unused_dependencies + self.misplaced_dependencies;
                let has_issues = (total_issues - self.fixed_dependencies) > 0;

                if has_issues {
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

                    if !self.options.fix {
                        let _ =
                            writeln!(self.writer, "To automatically fix issues, run with --fix");
                    }
                } else {
                    let _ = writeln!(self.writer, "No issues detected!");
                }

                ExitCode::from(u8::from(has_issues))
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

        let packages = metadata.workspace_packages();
        let packages: Vec<_> = packages
            .into_iter()
            .filter(|package| {
                // Skip if package is in the exclude list
                if self.options.exclude.iter().any(|name| name == package.name.as_str()) {
                    return false;
                }

                // Skip if specific packages are specified and this package is not in the list
                if !self.options.package.is_empty()
                    && !self.options.package.iter().any(|name| name == package.name.as_str())
                {
                    return false;
                }

                true
            })
            .collect();

        // Process packages in parallel
        let results: Vec<_> = packages
            .par_iter()
            .map(|package| {
                let manifest_path = package.manifest_path.as_std_path();
                let manifest = Manifest::from_path(manifest_path)?;
                let result = processor.process_package(&metadata, package, &manifest)?;
                Ok::<_, anyhow::Error>((*package, manifest_path, result))
            })
            .collect::<Result<Vec<_>>>()?;

        // Track all packages used across the workspace
        let mut workspace_used_pkgs = FxHashSet::default();

        for (package, manifest_path, result) in results {
            self.report_package_issues(package, &result, &metadata)?;
            self.fix_package_issues(manifest_path, &result)?;

            workspace_used_pkgs.extend(result.used_packages);
        }

        // Process workspace
        let manifest_path = metadata.workspace_root.as_std_path().join("Cargo.toml");
        let workspace_manifest = Manifest::from_path(&manifest_path)?;

        let workspace_result = PackageProcessor::process_workspace(
            &workspace_manifest,
            &metadata,
            &workspace_used_pkgs,
        );

        self.report_workspace_issues(&manifest_path, &workspace_result, &metadata)?;
        self.fix_workspace_issues(&manifest_path, &workspace_result)?;

        Ok(())
    }

    fn report_package_issues(
        &mut self,
        package: &Package,
        result: &PackageProcessResult,
        metadata: &Metadata,
    ) -> Result<()> {
        // Warn about redundant ignores
        for ignored_dep in &result.redundant_ignores {
            writeln!(
                self.writer,
                "warning: '{ignored_dep}' in [package.metadata.cargo-shear] for package '{}' is ignored but not needed; remove it unless you're suppressing a known false positive.\n",
                package.name
            )?;
        }

        let unused_count = result.unused_dependencies.len();
        let misplaced_count = result.misplaced_dependencies.len();

        if unused_count == 0 && misplaced_count == 0 {
            return Ok(());
        }

        let relative_path = PackageProcessor::get_relative_path(
            package.manifest_path.as_std_path(),
            metadata.workspace_root.as_std_path(),
        );

        writeln!(self.writer, "{} -- {}:", package.name, relative_path.display())?;

        if unused_count > 0 {
            writeln!(self.writer, "  unused dependencies:")?;
            for unused_dep in &result.unused_dependencies {
                writeln!(self.writer, "    {unused_dep}")?;
            }
        }

        if misplaced_count > 0 {
            writeln!(self.writer, "  move to dev-dependencies:")?;
            for misplaced_dep in &result.misplaced_dependencies {
                writeln!(self.writer, "    {misplaced_dep}")?;
            }
        }

        writeln!(self.writer)?;

        self.unused_dependencies += unused_count;
        self.misplaced_dependencies += misplaced_count;

        Ok(())
    }

    fn fix_package_issues(
        &mut self,
        manifest_path: &Path,
        result: &PackageProcessResult,
    ) -> Result<()> {
        if !self.options.fix {
            return Ok(());
        }

        if result.misplaced_dependencies.is_empty() && result.unused_dependencies.is_empty() {
            return Ok(());
        }

        let content = fs::read_to_string(manifest_path)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        let fixed_unused =
            CargoTomlEditor::remove_dependencies(&mut manifest, &result.unused_dependencies);
        let fixed_misplaced = CargoTomlEditor::move_to_dev_dependencies(
            &mut manifest,
            &result.misplaced_dependencies,
        );

        fs::write(manifest_path, manifest.to_string())?;
        self.fixed_dependencies += fixed_unused + fixed_misplaced;

        Ok(())
    }

    fn report_workspace_issues(
        &mut self,
        manifest_path: &Path,
        result: &WorkspaceProcessResult,
        metadata: &Metadata,
    ) -> Result<()> {
        // Warn about redundant workspace ignores
        for ignored_dep in &result.redundant_ignores {
            writeln!(
                self.writer,
                "warning: '{ignored_dep}' in [workspace.metadata.cargo-shear] is ignored but not needed; remove it unless you're suppressing a known false positive.\n"
            )?;
        }

        if result.unused_dependencies.is_empty() {
            return Ok(());
        }

        let path = PackageProcessor::get_relative_path(
            manifest_path,
            metadata.workspace_root.as_std_path(),
        );
        // Ensure relative paths start with ./ for consistency
        let path_str = if path.is_relative() && !path.starts_with(".") {
            format!("./{}", path.display())
        } else {
            path.display().to_string()
        };

        writeln!(self.writer, "root -- {path_str}:")?;
        writeln!(self.writer, "  unused dependencies:")?;
        for unused_dep in &result.unused_dependencies {
            writeln!(self.writer, "    {unused_dep}")?;
        }
        writeln!(self.writer)?;

        self.unused_dependencies += result.unused_dependencies.len();

        Ok(())
    }

    fn fix_workspace_issues(
        &mut self,
        manifest_path: &Path,
        result: &WorkspaceProcessResult,
    ) -> Result<()> {
        if !self.options.fix {
            return Ok(());
        }

        if result.unused_dependencies.is_empty() {
            return Ok(());
        }

        let content = fs::read_to_string(manifest_path)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        let fixed =
            CargoTomlEditor::remove_dependencies(&mut manifest, &result.unused_dependencies);

        fs::write(manifest_path, manifest.to_string())?;
        self.fixed_dependencies += fixed;

        Ok(())
    }
}
