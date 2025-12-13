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
//! - `package_analyzer` - Analyzes packages to find issues
//! - `package_processor` - Processes packages and detects unused dependencies
//! - `source_parser` - Parses Rust source to extract data
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
mod context;
mod diagnostics;
mod manifest;
mod output;
mod package_analyzer;
mod package_processor;
mod source_parser;
#[cfg(test)]
mod tests;
mod tree;
pub mod util;

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
use owo_colors::OwoColorize;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use toml_edit::DocumentMut;

pub use crate::output::{ColorMode, OutputFormat};
use crate::{
    cargo_toml_editor::CargoTomlEditor,
    context::{PackageContext, WorkspaceContext},
    diagnostics::ShearAnalysis,
    output::Renderer,
    package_processor::{PackageAnalysis, PackageProcessor, WorkspaceAnalysis},
    util::read_to_string,
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

    /// Output format: auto, json
    #[bpaf(long, fallback(OutputFormat::Auto))]
    format: OutputFormat,

    /// Color usage for output: auto, always, never
    #[bpaf(long, fallback(ColorMode::Auto))]
    color: ColorMode,

    /// Path to the project directory.
    ///
    /// Defaults to the current directory if not specified.
    #[bpaf(positional("PATH"), fallback_with(default_path))]
    path: PathBuf,
}

impl CargoShearOptions {
    /// Create new options with the given path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            fix: false,
            expand: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec![],
            format: OutputFormat::default(),
            color: ColorMode::default(),
        }
    }

    /// Enable fix mode.
    #[must_use]
    pub const fn with_fix(mut self) -> Self {
        self.fix = true;
        self
    }

    /// Enable macro expansion.
    #[must_use]
    pub const fn with_expand(mut self) -> Self {
        self.expand = true;
        self
    }

    /// Enable locked mode.
    #[must_use]
    pub const fn with_locked(mut self) -> Self {
        self.locked = true;
        self
    }

    /// Enable offline mode.
    #[must_use]
    pub const fn with_offline(mut self) -> Self {
        self.offline = true;
        self
    }

    /// Enable frozen mode.
    #[must_use]
    pub const fn with_frozen(mut self) -> Self {
        self.frozen = true;
        self
    }

    /// Set packages to check.
    #[must_use]
    pub fn with_packages(mut self, packages: Vec<String>) -> Self {
        self.package = packages;
        self
    }

    /// Set packages to exclude.
    #[must_use]
    pub fn with_excludes(mut self, excludes: Vec<String>) -> Self {
        self.exclude = excludes;
        self
    }

    /// Set output format.
    #[must_use]
    pub const fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Set color mode.
    #[must_use]
    pub const fn with_color(mut self, color: ColorMode) -> Self {
        self.color = color;
        self
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

    /// Result of the analysis.
    analysis: ShearAnalysis,
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
    pub fn new(writer: W, options: CargoShearOptions) -> Self {
        Self { writer, options, analysis: ShearAnalysis::default() }
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
        match self.shear() {
            Ok(()) => {
                let color = self.options.color.enabled();
                let mut renderer = Renderer::new(&mut self.writer, self.options.format, color);

                if let Err(err) = renderer.render(&self.analysis) {
                    let _ = writeln!(self.writer, "error rendering report: {err:?}");
                    return ExitCode::from(2);
                }

                if self.options.fix && self.analysis.fixed > 0 && self.analysis.errors == 0 {
                    ExitCode::SUCCESS
                } else if self.analysis.errors > 0 {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(err) => {
                let _ = writeln!(self.writer, "error: {err:?}");
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
            .verbose(true)
            .exec()
            .map_err(|e| anyhow::anyhow!("Metadata error: {e}"))?;

        let processor = PackageProcessor::new(self.options.expand);
        let workspace_ctx = WorkspaceContext::new(&metadata)?;

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

        let total = packages.len();
        let results: Vec<_> = if self.options.expand {
            // Process packages sequentially, since expand needs to invoke `cargo build`.
            packages
                .iter()
                .enumerate()
                .map(|(index, package)| {
                    eprintln!(
                        "{:>12} {} [{}/{}]",
                        "Expanding".bright_cyan().bold(),
                        package.name,
                        index + 1,
                        total
                    );

                    Self::process_package(&processor, &workspace_ctx, package, &metadata)
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            // Process packages in parallel
            packages
                .par_iter()
                .map(|package| {
                    Self::process_package(&processor, &workspace_ctx, package, &metadata)
                })
                .collect::<Result<Vec<_>>>()?
        };

        for (ctx, result) in results {
            let fixed = self.fix_package_issues(&ctx.manifest_path, &result)?;
            self.analysis.add_package_result(&ctx, &result, fixed);
        }

        // Only analyze workspace if we're targeting all packages.
        if self.options.package.is_empty() && self.options.exclude.is_empty() {
            let workspace_result =
                PackageProcessor::process_workspace(&workspace_ctx, &self.analysis.packages);

            let fixed =
                self.fix_workspace_issues(&workspace_ctx.manifest_path, &workspace_result)?;
            self.analysis.add_workspace_result(&workspace_ctx, &workspace_result, fixed);
        }

        Ok(())
    }

    fn process_package<'a>(
        processor: &PackageProcessor,
        workspace_ctx: &'a WorkspaceContext,
        package: &Package,
        metadata: &'a Metadata,
    ) -> Result<(PackageContext<'a>, PackageAnalysis)> {
        let ctx = PackageContext::new(workspace_ctx, package, metadata)?;
        let result = processor.process_package(&ctx)?;
        Ok((ctx, result))
    }

    fn fix_package_issues(&self, manifest_path: &Path, result: &PackageAnalysis) -> Result<usize> {
        if !self.options.fix {
            return Ok(0);
        }

        if result.misplaced_dependencies.is_empty() && result.unused_dependencies.is_empty() {
            return Ok(0);
        }

        let content = read_to_string(manifest_path)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        let fixed_unused =
            CargoTomlEditor::remove_dependencies(&mut manifest, &result.unused_dependencies);
        let fixed_misplaced = CargoTomlEditor::move_to_dev_dependencies(
            &mut manifest,
            &result.misplaced_dependencies,
        );

        fs::write(manifest_path, manifest.to_string())?;
        Ok(fixed_unused + fixed_misplaced)
    }

    fn fix_workspace_issues(
        &self,
        manifest_path: &Path,
        result: &WorkspaceAnalysis,
    ) -> Result<usize> {
        if !self.options.fix {
            return Ok(0);
        }

        if result.unused_dependencies.is_empty() {
            return Ok(0);
        }

        let content = read_to_string(manifest_path)?;
        let mut manifest = DocumentMut::from_str(&content)?;

        let fixed =
            CargoTomlEditor::remove_workspace_deps(&mut manifest, &result.unused_dependencies);

        fs::write(manifest_path, manifest.to_string())?;
        Ok(fixed)
    }
}
