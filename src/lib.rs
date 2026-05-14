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
//! let options = CargoShearOptions::new(std::path::PathBuf::from("."));
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
use rustc_hash::FxHashSet;
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
/// Parsed from `argv` by `bpaf`. The doc comments on individual fields below
/// become the `--help` text Cargo's users see, so keep them user-facing.
///
/// The "batteries" `cargo_helper` strips the leading `shear` argument when
/// invoked as `cargo shear`, so this struct sees the same shape either way.
/// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>.
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

    /// Detect mismatches between `test`/`doctest` target settings and source content.
    ///
    /// When set, cargo-shear warns when `[lib] test = false` is paired with source
    /// that contains tests (or `doctest = false` with source that contains doc tests),
    /// and — within a workspace — when `test`/`doctest` are left at their default of
    /// `true` for lib targets that contain none.
    #[bpaf(long("check-test-targets"))]
    check_test_targets: bool,

    /// Treat warnings as errors.
    ///
    /// When set, warnings will cause cargo-shear to exit with a failure code.
    #[bpaf(long("deny-warnings"))]
    deny_warnings: bool,

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

    /// Output format: auto, json, github
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
    /// Construct an options value with every flag at its default and the project
    /// rooted at `path`. Programmatic callers usually layer `with_*` builders on top.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            fix: false,
            expand: false,
            check_test_targets: false,
            deny_warnings: false,
            locked: false,
            offline: false,
            frozen: false,
            package: vec![],
            exclude: vec![],
            format: OutputFormat::default(),
            color: ColorMode::default(),
        }
    }

    /// Enable `--fix`: rewrite manifests to apply fixable diagnostics.
    #[must_use]
    pub const fn with_fix(mut self) -> Self {
        self.fix = true;
        self
    }

    /// Enable `--expand`: run `cargo expand` first to surface macro-generated imports.
    #[must_use]
    pub const fn with_expand(mut self) -> Self {
        self.expand = true;
        self
    }

    /// Enable `--check-test-targets`: emit `test`/`doctest` flag mismatch diagnostics.
    #[must_use]
    pub const fn with_check_test_targets(mut self) -> Self {
        self.check_test_targets = true;
        self
    }

    /// Enable `--deny-warnings`: turn any warning into a non-zero exit.
    #[must_use]
    pub const fn with_deny_warnings(mut self) -> Self {
        self.deny_warnings = true;
        self
    }

    /// Enable `--locked`: forwarded to `cargo metadata`.
    #[must_use]
    pub const fn with_locked(mut self) -> Self {
        self.locked = true;
        self
    }

    /// Enable `--offline`: forwarded to `cargo metadata`.
    #[must_use]
    pub const fn with_offline(mut self) -> Self {
        self.offline = true;
        self
    }

    /// Enable `--frozen`: forwarded to `cargo metadata` (implies `--locked` and `--offline`).
    #[must_use]
    pub const fn with_frozen(mut self) -> Self {
        self.frozen = true;
        self
    }

    /// Restrict the run to these workspace members. Empty means "all".
    #[must_use]
    pub fn with_packages(mut self, packages: Vec<String>) -> Self {
        self.package = packages;
        self
    }

    /// Exclude these workspace members from the run.
    #[must_use]
    pub fn with_excludes(mut self, excludes: Vec<String>) -> Self {
        self.exclude = excludes;
        self
    }

    /// Pick the renderer used for diagnostics.
    #[must_use]
    pub const fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Override automatic color detection.
    #[must_use]
    pub const fn with_color(mut self, color: ColorMode) -> Self {
        self.color = color;
        self
    }

    /// Apply environment-based resolution to defaults — currently only
    /// substitutes the GitHub renderer for `format = Auto` under CI.
    /// Call this after CLI parsing but before passing options to `CargoShear`.
    #[must_use]
    pub fn resolve(mut self) -> Self {
        self.format = self.format.resolve();
        self
    }
}

pub(crate) fn default_path() -> Result<PathBuf> {
    Ok(env::current_dir()?)
}

/// Top-level entry point: orchestrates `cargo metadata`, runs each package
/// through [`PackageProcessor`], optionally rewrites manifests, and renders
/// the aggregated [`ShearAnalysis`] to `writer`.
pub struct CargoShear<W> {
    /// Sink for rendered diagnostics (typically `std::io::stdout()`).
    writer: W,

    /// Caller-supplied configuration; immutable once `run` starts.
    options: CargoShearOptions,

    /// Diagnostics accumulated as each package is processed.
    analysis: ShearAnalysis,
}

impl<W: Write> CargoShear<W> {
    /// Build a runner that will write diagnostics to `writer` using the
    /// settings in `options`.
    ///
    /// ```
    /// use cargo_shear::{CargoShear, CargoShearOptions};
    /// use std::path::PathBuf;
    ///
    /// let options = CargoShearOptions::new(PathBuf::from("."));
    /// let shear = CargoShear::new(std::io::stdout(), options);
    /// ```
    #[must_use]
    pub fn new(writer: W, options: CargoShearOptions) -> Self {
        let analysis = ShearAnalysis::new(options.clone());
        Self { writer, options, analysis }
    }

    /// Execute the full pipeline: analyze every selected package, optionally
    /// apply `--fix` rewrites, render the report, and translate the result
    /// into a process exit code.
    ///
    /// Returns:
    /// - `0` if no issues were found, or every found issue was fixed by `--fix`.
    /// - `1` if issues were found and not all of them were fixed.
    /// - `2` if a fatal error occurred (cargo failure, IO error, ...).
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

                self.determine_exit_code()
            }
            Err(err) => {
                let _ = writeln!(self.writer, "error: {err:?}");
                ExitCode::from(2)
            }
        }
    }

    /// Translate accumulated counts into the documented `0`/`1`/`2` exit code.
    /// `2` is reserved for fatal errors and is set in [`run`](Self::run).
    const fn determine_exit_code(&self) -> ExitCode {
        // `--fix` succeeded if at least one repair landed and nothing else errored.
        if self.options.fix && self.analysis.fixed > 0 && self.analysis.errors == 0 {
            return ExitCode::SUCCESS;
        }

        // Errors always fail; warnings only fail when `--deny-warnings` was passed.
        let has_errors = self.analysis.errors > 0;
        let has_warnings = self.options.deny_warnings && self.analysis.warnings > 0;

        if has_errors || has_warnings { ExitCode::FAILURE } else { ExitCode::SUCCESS }
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

        let processor = PackageProcessor::new(self.options.expand, self.options.check_test_targets);
        let workspace_ctx = WorkspaceContext::new(&metadata)?;

        let packages = metadata.workspace_packages();
        let packages: Vec<_> = packages
            .into_iter()
            .filter(|package| {
                // `--exclude` always wins over `--package` (matches Cargo's behaviour).
                if self.options.exclude.iter().any(|name| name == package.name.as_str()) {
                    return false;
                }

                // When `--package` is given, restrict the run to its allowlist.
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
            // `cargo expand` shells out to `cargo build` per package, which holds
            // a global build lock — running these in parallel would just serialize
            // on the lock and lose the progress output. Stay sequential.
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

        let mut used_workspace_ignore_paths: FxHashSet<String> = FxHashSet::default();
        for (ctx, result) in results {
            let fixed = self.fix_package_issues(&ctx.manifest_path, &result)?;
            used_workspace_ignore_paths.extend(result.used_workspace_ignore_paths.iter().cloned());
            self.analysis.add_package_result(&ctx, &result, fixed);
        }

        // Workspace-level diagnostics need a complete view of which workspace
        // dependencies are actually used; skip them when the user has narrowed
        // the run with `--package`/`--exclude` and the picture is incomplete.
        if self.options.package.is_empty() && self.options.exclude.is_empty() {
            let workspace_result = PackageProcessor::process_workspace(
                &workspace_ctx,
                &self.analysis.packages,
                &used_workspace_ignore_paths,
            );

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

        if !result.has_fixable_issues() {
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
        let mut flag_fixes = 0usize;
        if !result.test_disabled_with_tests.is_empty() {
            flag_fixes += usize::from(CargoTomlEditor::remove_lib_flag(&mut manifest, "test"));
        }
        if !result.test_enabled_without_tests.is_empty() {
            CargoTomlEditor::set_lib_flag_false(&mut manifest, "test");
            flag_fixes += 1;
        }
        if !result.doctest_disabled_with_doctests.is_empty() {
            flag_fixes += usize::from(CargoTomlEditor::remove_lib_flag(&mut manifest, "doctest"));
        }
        if !result.doctest_enabled_without_doctests.is_empty() {
            CargoTomlEditor::set_lib_flag_false(&mut manifest, "doctest");
            flag_fixes += 1;
        }

        fs::write(manifest_path, manifest.to_string())?;
        Ok(fixed_unused + fixed_misplaced + flag_fixes)
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
