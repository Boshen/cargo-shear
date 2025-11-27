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
pub mod diagnostic;
mod import_collector;
mod manifest;
mod package_processor;
#[cfg(test)]
mod tests;

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use anyhow::Result;
use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, MetadataCommand};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashSet;
use toml_edit::DocumentMut;

use crate::{
    cargo_toml_editor::CargoTomlEditor,
    diagnostic::{
        BoxedDiagnostic, DiagnosticPrinter, FixHelp, IgnoreHelpPackage, IgnoreHelpWorkspace,
        ProcessingError,
    },
    manifest::Manifest,
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

    /// Disable colored output.
    #[bpaf(long("no-color"), flag(false, true), fallback(true))]
    color: bool,

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
            color: false,
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

    /// Collected diagnostics
    diagnostics: Vec<BoxedDiagnostic>,

    /// Whether any fixable issues were found
    has_fixable_issues: bool,

    /// Whether any fixes were applied
    has_fixed: bool,
}

impl<W: io::Write> CargoShear<W> {
    /// Create a new `CargoShear` instance with the given options.
    ///
    /// # Arguments
    ///
    /// * `writer` - Writer for output
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
            diagnostics: Vec::new(),
            has_fixable_issues: false,
            has_fixed: false,
        }
    }

    /// Run the dependency analysis and optionally fix issues.
    ///
    /// This method performs the complete analysis workflow:
    /// 1. Analyzes all packages in the workspace
    /// 2. Detects issues
    /// 3. Optionally fixes them if `--fix` is enabled
    /// 4. Prints diagnostics to the writer
    ///
    /// # Returns
    ///
    /// - Exit code `0` if no issues were found or all issues were fixed
    /// - Exit code `1` if issues were found (without `--fix`)
    /// - Exit code `2` if an error occurred
    pub fn run(mut self) -> ExitCode {
        let printer = if self.options.color {
            DiagnosticPrinter::fancy()
        } else {
            DiagnosticPrinter::plain()
        };

        match self.shear() {
            Ok(()) => {
                let has_issues = self.has_fixable_issues && !self.has_fixed;

                for diagnostic in &self.diagnostics {
                    let _ = printer.print(diagnostic.as_ref(), &mut self.writer);
                }
                if has_issues {
                    let _ = printer.print(&IgnoreHelpPackage, &mut self.writer);
                    let _ = printer.print(&IgnoreHelpWorkspace, &mut self.writer);
                    if !self.options.fix {
                        let _ = printer.print(&FixHelp, &mut self.writer);
                    }
                }

                ExitCode::from(u8::from(if self.options.fix { self.has_fixed } else { has_issues }))
            }
            Err(err) => {
                let _ = printer.print(&ProcessingError::new(err), &mut self.writer);
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

        let workspace_root = metadata.workspace_root.as_std_path().to_path_buf();
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

        let workspace_manifest_path = metadata.workspace_root.as_std_path().join("Cargo.toml");
        let workspace_manifest_content = fs::read_to_string(&workspace_manifest_path)?;
        let workspace_manifest: Manifest = toml::from_str(&workspace_manifest_content)?;

        // Process packages in parallel
        let results: Vec<_> = packages
            .par_iter()
            .map(|package| {
                let manifest_path = package.manifest_path.as_std_path();
                let manifest_content = fs::read_to_string(manifest_path)?;
                let manifest: Manifest = toml::from_str(&manifest_content)?;
                let result = processor.process_package(
                    &metadata,
                    package,
                    &manifest,
                    &manifest_content,
                    &workspace_manifest,
                    &workspace_root,
                )?;
                Ok::<_, anyhow::Error>((manifest_path, result))
            })
            .collect::<Result<Vec<_>>>()?;

        // Track all packages used across the workspace
        let mut workspace_used_pkgs = FxHashSet::default();

        for (manifest_path, mut result) in results {
            self.fix_package_issues(manifest_path, &result)?;
            self.report_package_issues(&mut result);

            workspace_used_pkgs.extend(result.used_packages);
        }

        // Process workspace
        let mut workspace_result = PackageProcessor::process_workspace(
            &workspace_manifest_path,
            &workspace_manifest,
            &workspace_manifest_content,
            &metadata,
            &workspace_used_pkgs,
            &workspace_root,
        );

        self.fix_workspace_issues(&workspace_manifest_path, &workspace_result)?;
        self.report_workspace_issues(&mut workspace_result);

        Ok(())
    }

    fn report_package_issues(&mut self, result: &mut PackageProcessResult) {
        if !result.has_issues() {
            return;
        }

        self.has_fixable_issues |= result.has_fixable_issues();
        self.diagnostics.extend(result.diagnostics());
    }

    fn fix_package_issues(
        &mut self,
        manifest_path: &Path,
        result: &PackageProcessResult,
    ) -> Result<()> {
        if !self.options.fix || !result.has_fixable_issues() {
            return Ok(());
        }

        let content = fs::read_to_string(&result.manifest)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        CargoTomlEditor::remove_dependencies(&mut manifest, &result.unused);
        CargoTomlEditor::move_to_dev_dependencies(&mut manifest, &result.misplaced);

        fs::write(manifest_path, manifest.to_string())?;
        self.has_fixed = true;

        Ok(())
    }

    fn report_workspace_issues(&mut self, result: &mut WorkspaceProcessResult) {
        if !result.has_issues() {
            return;
        }

        self.has_fixable_issues |= result.has_fixable_issues();
        self.diagnostics.extend(result.diagnostics());
    }

    fn fix_workspace_issues(
        &mut self,
        manifest_path: &Path,
        result: &WorkspaceProcessResult,
    ) -> Result<()> {
        if !self.options.fix || !result.has_fixable_issues() {
            return Ok(());
        }

        let content = fs::read_to_string(&result.manifest)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        CargoTomlEditor::remove_workspace_deps(&mut manifest, &result.unused);

        fs::write(manifest_path, manifest.to_string())?;
        self.has_fixed = true;

        Ok(())
    }
}
