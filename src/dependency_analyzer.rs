//! Dependency analysis module for cargo-shear.
//!
//! This module is responsible for analyzing Rust source code to determine
//! which dependencies are actually used. It supports two modes:
//!
//! 1. **Normal mode**: Parses Rust source files directly using `syn`
//! 2. **Expand mode**: Uses `cargo expand` to expand macros for more accurate detection
//!
//! The analyzer walks through all source files in a package, collects import
//! statements, and builds a set of used dependency names.

use rustc_hash::FxHashSet as HashSet;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{Package, TargetKind};
use cargo_util_schemas::core::PackageIdSpec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::{DirEntry, WalkDir};

use crate::import_collector::collect_imports;
use anyhow::{Result, anyhow};

/// A set of dependency names (crate names).
pub type Dependencies = HashSet<String>;

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
    ///
    /// # Arguments
    ///
    /// * `expand_macros` - If true, use `cargo expand` for more accurate analysis
    pub const fn new(expand_macros: bool) -> Self {
        Self { expand_macros }
    }

    /// Analyze a package to find all used dependencies.
    ///
    /// This method will either parse source files directly or use cargo expand
    /// based on the `expand_macros` setting.
    ///
    /// # Arguments
    ///
    /// * `package` - The package to analyze
    ///
    /// # Returns
    ///
    /// A set of dependency names that are used in the package's source code
    pub fn analyze_package(&self, package: &Package) -> Result<Dependencies> {
        if self.expand_macros {
            Self::analyze_with_expansion(package)
        } else {
            Self::analyze_from_files(package)
        }
    }

    fn analyze_from_files(package: &Package) -> Result<Dependencies> {
        let rust_files = Self::get_package_rust_files(package);

        let deps_vec: Vec<Dependencies> = rust_files
            .par_iter()
            .map(|path| Self::process_rust_source(path))
            .collect::<Result<Vec<_>>>()?;

        Ok(deps_vec.into_iter().fold(HashSet::default(), |a, b| a.union(&b).cloned().collect()))
    }

    fn analyze_with_expansion(package: &Package) -> Result<Dependencies> {
        let mut combined_imports = Self::analyze_from_files(package)?;

        for target in &package.targets {
            let target_arg =
                match target.kind.first().ok_or_else(|| anyhow!("Failed to get target kind"))? {
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
                    "Cargo expand failed for {}: {}",
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

            let imports = collect_imports(&output_str).map_err(|e| anyhow!("Syntax error: {e}"))?;
            combined_imports.extend(imports);
        }

        Ok(combined_imports)
    }

    fn get_package_rust_files(package: &Package) -> Vec<PathBuf> {
        package
            .targets
            .iter()
            .flat_map(|target| {
                if target.kind.contains(&TargetKind::CustomBuild) {
                    vec![target.src_path.clone().into_std_path_buf()]
                } else {
                    let target_dir = target.src_path.parent().unwrap_or_else(|| {
                        panic!("Failed to get parent path {}", &target.src_path)
                    });

                    WalkDir::new(target_dir)
                        .into_iter()
                        .filter_map(std::result::Result::ok)
                        .filter(|e| {
                            e.file_type().is_file()
                                && e.path().extension().is_some_and(|ext| ext == "rs")
                        })
                        .map(DirEntry::into_path)
                        .collect::<Vec<_>>()
                }
            })
            .collect()
    }

    fn process_rust_source(path: &Path) -> Result<Dependencies> {
        let source_text = std::fs::read_to_string(path)?;
        collect_imports(&source_text).map_err(|e| anyhow!("Syntax error: {e}"))
    }

    /// Parse a package ID string to extract the package name.
    ///
    /// Package IDs can have different formats depending on the Rust version:
    /// - Pre-1.77: `memchr 2.7.1 (registry+https://github.com/rust-lang/crates.io-index)`
    /// - 1.77+: `registry+https://github.com/rust-lang/crates.io-index#memchr@2.7.1`
    ///
    /// # Arguments
    ///
    /// * `s` - The package ID string to parse
    ///
    /// # Returns
    ///
    /// The extracted package name
    pub fn parse_package_id(s: &str) -> Result<String> {
        if s.contains(' ') {
            s.split(' ')
                .next()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Parse error: {s} should have a space"))
        } else {
            PackageIdSpec::parse(s)
                .map(|id| id.name().to_owned())
                .map_err(|e| anyhow!("Parse error: {e}"))
        }
    }

    /// Extract the list of ignored package names from metadata.
    ///
    /// Looks for package names in the `cargo-shear.ignored` field of the metadata JSON.
    /// These packages will be excluded from unused dependency detection.
    ///
    /// # Arguments
    ///
    /// * `value` - The metadata JSON value (usually from package or workspace metadata)
    ///
    /// # Returns
    ///
    /// A set of package names to ignore
    pub fn get_ignored_package_names(value: &serde_json::Value) -> HashSet<&str> {
        value
            .as_object()
            .and_then(|object| object.get("cargo-shear"))
            .and_then(|object| object.get("ignored"))
            .and_then(|ignored| ignored.as_array())
            .map(|ignored| ignored.iter().filter_map(|item| item.as_str()).collect::<HashSet<_>>())
            .unwrap_or_default()
    }
}
