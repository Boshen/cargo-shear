//! Dependency analysis module for cargo-shear.
//!
//! This module is responsible for analyzing Rust source code to determine
//! which dependencies are actually used. It supports two modes:
//!
//! 1. **Normal mode**: Parses Rust source files directly using `ra_ap_syntax`
//! 2. **Expand mode**: Uses `cargo expand` to expand macros for more accurate detection
//!
//! The analyzer walks through all source files in a package, collects import
//! statements, and builds a set of used import names.

use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Result, anyhow};
use cargo_metadata::{Package, Target, TargetKind};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use walkdir::{DirEntry, WalkDir};

use crate::{
    import_collector::collect_imports,
    manifest::{FeatureDep, Manifest},
};

/// Categorized imports based on where they are used.
#[derive(Debug, Default)]
pub struct CategorizedImports {
    /// Imports used in normal targets (lib, bin, ...).
    pub normal: FxHashSet<String>,

    /// Imports used in dev targets (test, bench, example).
    pub dev: FxHashSet<String>,

    /// Imports used in build scripts.
    pub build: FxHashSet<String>,

    /// Features referencing each import (maps import name to features).
    pub features: FxHashMap<String, Vec<FeatureDep>>,
}

impl CategorizedImports {
    pub fn all_imports(&self) -> FxHashSet<String> {
        self.normal
            .union(&self.dev)
            .chain(&self.build)
            .chain(self.features.keys())
            .cloned()
            .collect()
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
        let mut categorized = CategorizedImports::default();

        if self.expand_macros {
            Self::analyze_with_expansion(&mut categorized, package)?;
        } else {
            Self::analyze_from_files(&mut categorized, package)?;
        }

        Self::analyze_features(&mut categorized, manifest);
        Ok(categorized)
    }

    fn analyze_from_files(categorized: &mut CategorizedImports, package: &Package) -> Result<()> {
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

            Self::categorize_imports(categorized, target_kind, imports);
        }

        Ok(())
    }

    fn analyze_with_expansion(
        categorized: &mut CategorizedImports,
        package: &Package,
    ) -> Result<()> {
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

            let imports = collect_imports(&output_str);
            Self::categorize_imports(categorized, target_kind, imports);
        }

        Ok(())
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
        Ok(collect_imports(&source_text))
    }

    /// Collect feature references for dependencies referenced in `[features]`.
    ///
    /// This handles:
    /// - Explicit features (e.g. `["dep:foo"]`)
    /// - Feature enablement (e.g. `["foo/std"]`)
    /// - Weak feature enablement (e.g. `["foo?/std"]`)
    ///
    /// Implicit features (e.g. `foo = { optional = true }`) are not tracked here.
    fn analyze_features(categorized: &mut CategorizedImports, manifest: &Manifest) {
        for (key, values) in &manifest.features {
            let name = key.get_ref();
            for value in values {
                let feature = FeatureDep { name: name.clone(), span: value.span() };
                let value = value.get_ref();

                if let Some(dep) = value.strip_prefix("dep:") {
                    let import = dep.replace('-', "_");
                    categorized.features.entry(import).or_default().push(feature);
                    continue;
                }

                if let Some((dep, _)) = value.split_once('/') {
                    let dep = dep.trim_end_matches('?');
                    let import = dep.replace('-', "_");
                    categorized.features.entry(import).or_default().push(feature);
                }
            }
        }
    }
}
