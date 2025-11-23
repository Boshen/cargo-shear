//! Dependency analysis module for cargo-shear.
//!
//! This module is responsible for analyzing Rust source code to determine
//! which dependencies are actually used. It supports two modes:
//!
//! 1. **Normal mode**: Parses Rust source files directly using `syn`
//! 2. **Expand mode**: Uses `cargo expand` to expand macros for more accurate detection
//!
//! The analyzer walks through all source files in a package, collects import
//! statements, and builds a set of used import names.

use std::{
    env,
    ffi::OsString,
    fmt::Write,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Result, anyhow};
use cargo_metadata::{Package, Target, TargetKind};
use cargo_toml::Manifest;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashSet;
use walkdir::{DirEntry, WalkDir};

use crate::import_collector::collect_imports;

/// Categorized imports based on where they are used.
#[derive(Debug, Default)]
pub struct CategorizedImports {
    /// Imports used in normal targets (lib, bin, ...).
    pub normal: FxHashSet<String>,

    /// Imports used in dev targets (test, bench, example).
    pub dev: FxHashSet<String>,

    /// Imports used in build scripts.
    pub build: FxHashSet<String>,
}

impl CategorizedImports {
    pub fn all_imports(&self) -> FxHashSet<String> {
        self.normal.union(&self.dev).chain(&self.build).cloned().collect()
    }
}

/// Analyzes Rust source code to find used dependencies.
///
/// The analyzer can operate in two modes:
/// - Normal: Parses source files directly (faster but may miss macro-generated imports)
/// - Expand: Uses cargo expand to expand macros first (slower but more accurate)
pub struct DependencyAnalyzer {
    /// Whether to use cargo expand to expand macros
    expand_macros: bool,
}

impl DependencyAnalyzer {
    /// Create a new dependency analyzer.
    pub const fn new(expand_macros: bool) -> Self {
        Self { expand_macros }
    }

    /// Analyze a package to find all used imports, categorized by target type.
    pub fn analyze_package(
        &self,
        package: &Package,
        manifest: &Manifest,
    ) -> Result<CategorizedImports> {
        let mut categorized = if self.expand_macros {
            Self::analyze_with_expansion(package)?
        } else {
            Self::analyze_from_files(package)?
        };

        // Features can only be normal dependencies
        let features = Self::analyze_features(manifest);
        categorized.normal.extend(features);

        Ok(categorized)
    }

    fn analyze_from_files(package: &Package) -> Result<CategorizedImports> {
        let mut categorized = CategorizedImports::default();

        for target in &package.targets {
            let target_kind = target.kind.first().ok_or_else(|| anyhow!("Target has no kind"))?;
            let rust_files = Self::get_target_rust_files(target);

            let deps_vec: Vec<FxHashSet<String>> = rust_files
                .par_iter()
                .map(|path| Self::process_rust_source(path))
                .collect::<Result<Vec<_>>>()?;

            let imports = deps_vec
                .into_iter()
                .fold(FxHashSet::default(), |a, b| a.union(&b).cloned().collect());

            Self::categorize_imports(&mut categorized, target_kind, imports);
        }

        Ok(categorized)
    }

    fn analyze_with_expansion(package: &Package) -> Result<CategorizedImports> {
        let mut categorized = Self::analyze_from_files(package)?;

        for target in &package.targets {
            let target_kind =
                target.kind.first().ok_or_else(|| anyhow!("Failed to get target kind"))?;

            let target_arg = match target_kind {
                TargetKind::CustomBuild => continue,
                TargetKind::Bin => format!("--bin={}", target.name),
                TargetKind::Example => format!("--example={}", target.name),
                TargetKind::Test => format!("--test={}", target.name),
                TargetKind::Bench => format!("--bench={}", target.name),
                TargetKind::CDyLib
                | TargetKind::DyLib
                | TargetKind::Lib
                | TargetKind::ProcMacro
                | TargetKind::RLib
                | TargetKind::StaticLib
                | TargetKind::Unknown(_)
                | _ => "--lib".to_owned(),
            };

            let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

            let mut cmd = Command::new(cargo);
            cmd.arg("rustc")
                .arg(&target_arg)
                .arg("--all-features")
                .arg("--profile=check")
                .arg("--color=never")
                .arg("--")
                .arg("-Zunpretty=expanded")
                .current_dir(package.manifest_path.parent().ok_or_else(|| {
                    anyhow!("Failed to get parent path: {}", package.manifest_path)
                })?);

            let output = cmd.output()?;
            if !output.status.success() {
                return Err(anyhow!(
                    "Cargo expand failed for {}:\n{}",
                    target.name,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            let output_str = String::from_utf8(output.stdout)?;
            if output_str.is_empty() {
                return Err(anyhow!(
                    "Cargo expand failed for {}: Empty output from cargo expand",
                    target.name
                ));
            }

            let imports = collect_imports(&output_str).map_err(|err| {
                let location = err.span().start();
                let snippet = Self::extract_code_snippet(&output_str, location.line);

                anyhow!(
                    "Syntax error in {} at line {}:{}:\n{err}\n{snippet}",
                    target.name,
                    location.line,
                    location.column
                )
            })?;

            Self::categorize_imports(&mut categorized, target_kind, imports);
        }

        Ok(categorized)
    }

    /// Collect all Rust source files for a target.
    fn get_target_rust_files(target: &Target) -> Vec<PathBuf> {
        if target.kind.contains(&TargetKind::CustomBuild) {
            vec![target.src_path.clone().into_std_path_buf()]
        } else {
            let target_dir = target
                .src_path
                .parent()
                .unwrap_or_else(|| panic!("Failed to get parent path {}", &target.src_path));

            WalkDir::new(target_dir)
                .into_iter()
                .filter_map(std::result::Result::ok)
                .filter(|e| {
                    e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "rs")
                })
                .map(DirEntry::into_path)
                .collect::<Vec<_>>()
        }
    }

    /// Categorize imports into normal, dev, or build based on target kind.
    fn categorize_imports(
        categorized: &mut CategorizedImports,
        target_kind: &TargetKind,
        imports: FxHashSet<String>,
    ) {
        match target_kind {
            TargetKind::CustomBuild => categorized.build.extend(imports),
            TargetKind::Test | TargetKind::Bench | TargetKind::Example => {
                categorized.dev.extend(imports);
            }
            TargetKind::Lib
            | TargetKind::Bin
            | TargetKind::CDyLib
            | TargetKind::DyLib
            | TargetKind::ProcMacro
            | TargetKind::RLib
            | TargetKind::StaticLib
            | TargetKind::Unknown(_)
            | _ => categorized.normal.extend(imports),
        }
    }

    /// Parse a Rust source file and collect all import names.
    fn process_rust_source(path: &Path) -> Result<FxHashSet<String>> {
        let source_text = std::fs::read_to_string(path)?;
        collect_imports(&source_text).map_err(|err| {
            let location = err.span().start();
            let snippet = Self::extract_code_snippet(&source_text, location.line);

            anyhow!(
                "Syntax error in {} at line {}:{}:\n{err}\n{snippet}",
                path.display(),
                location.line,
                location.column
            )
        })
    }

    /// Extracts a snippet of code around the specified line number.
    fn extract_code_snippet(source: &str, location: usize) -> String {
        let lines: Vec<&str> = source.lines().collect();
        let total = lines.len();

        if location == 0 || location > total {
            return String::new();
        }

        // Try and show 3 lines of context before/after the location
        let start = location.saturating_sub(4);
        let end = (location + 3).min(total);

        let mut snippet = String::from("\n");
        for (index, line) in lines.iter().enumerate().skip(start).take(end - start) {
            let line_num = index + 1;
            let marker = if line_num == location { ">" } else { " " };
            let _ = writeln!(snippet, "{marker} {line_num:4} | {line}");
        }

        snippet
    }

    /// Collect import names for dependencies referenced in features.
    ///
    /// This handles:
    /// - Explicit features (e.g. `["dep:foo"]`)
    /// - Feature enablement (e.g. `["foo/std"]`)
    /// - Weak feature enablement (e.g. `["foo?/std"]`)
    /// - Implicit dependencies (e.g. `foo = { optional = true }`)
    ///
    /// We convert these dependency keys into imports, in order to simplify merging with discovered source code imports.
    fn analyze_features(manifest: &Manifest) -> FxHashSet<String> {
        let mut imports = FxHashSet::default();

        // Collect explicit features
        for features in manifest.features.values() {
            for feature in features {
                if let Some(dep) = feature.strip_prefix("dep:") {
                    let import = dep.replace('-', "_");
                    imports.insert(import);
                    continue;
                }

                if let Some((dep, _)) = feature.split_once('/') {
                    let dep = dep.trim_end_matches('?');
                    let import = dep.replace('-', "_");
                    imports.insert(import);
                }
            }
        }

        // Collect implicit features from optional dependencies
        for (dep, details) in &manifest.dependencies {
            if details.optional() {
                let import = dep.replace('-', "_");
                imports.insert(import);
            }
        }

        imports
    }

    /// Extract the list of ignored deps from metadata.
    pub fn get_ignored_dependency_keys(value: &serde_json::Value) -> FxHashSet<&str> {
        value
            .as_object()
            .and_then(|object| object.get("cargo-shear"))
            .and_then(|object| object.get("ignored"))
            .and_then(|ignored| ignored.as_array())
            .map(|ignored| {
                ignored.iter().filter_map(|item| item.as_str()).collect::<FxHashSet<_>>()
            })
            .unwrap_or_default()
    }
}
