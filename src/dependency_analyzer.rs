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

use std::{env, ffi::OsString, path::PathBuf, process::Command};

use anyhow::{Result, anyhow};
use cargo_metadata::{Package, Target, TargetKind};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use toml::Spanned;
use walkdir::{DirEntry, WalkDir};

use crate::{manifest::Manifest, source_parser::ParsedSource};

/// How a dependency is referenced in `[features]`.
///
/// See: <https://doc.rust-lang.org/cargo/reference/features.html>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FeatureRef {
    /// Implicit feature from an optional dependency.
    /// Cargo only creates an implicit feature for an optional dependency if there are no `dep:` references to it.
    ///
    /// ```toml
    /// [dependencies]
    /// foo = { version = "1.0", optional = true }
    /// ```
    Implicit,

    /// Explicit dependency reference.
    ///
    /// ```toml
    /// [features]
    /// feature = ["dep:foo"]
    /// ```
    ///
    /// ```toml
    /// [features]
    /// feature = ["foo"]
    /// ```
    Explicit { feature: Spanned<String>, value: Spanned<String> },

    /// Dependency feature enablement.
    ///
    /// ```toml
    /// [features]
    /// feature = ["foo/bar"]
    /// ```
    DepFeature { feature: Spanned<String>, value: Spanned<String> },

    /// Weak dependency feature enablement.
    ///
    /// ```toml
    /// [features]
    /// feature = ["foo?/bar"]
    /// ```
    WeakDepFeature { feature: Spanned<String>, value: Spanned<String> },
}

impl FeatureRef {
    fn parse(feature: &Spanned<String>, value: &Spanned<String>) -> (String, Self) {
        // Handle `dep:foo` syntax
        if let Some(dep) = value.as_ref().strip_prefix("dep:") {
            let import = dep.replace('-', "_");
            return (import, Self::Explicit { feature: feature.clone(), value: value.clone() });
        }

        // Handle `foo/bar` and `foo?/bar` syntax
        if let Some((dep, _)) = value.as_ref().split_once('/') {
            let is_weak = dep.ends_with('?');

            let dep = dep.trim_end_matches('?');
            let import = dep.replace('-', "_");

            let feature = if is_weak {
                Self::WeakDepFeature { feature: feature.clone(), value: value.clone() }
            } else {
                Self::DepFeature { feature: feature.clone(), value: value.clone() }
            };

            return (import, feature);
        }

        // Assume this is enabling another feature.
        // May be a dependency, so worth tracking.
        let import = value.as_ref().replace('-', "_");
        (import, Self::Explicit { feature: feature.clone(), value: value.clone() })
    }
}

/// Categorized imports based on where they are used.
#[derive(Debug, Default)]
pub struct CategorizedImports {
    /// Imports used in normal targets (lib, bin, ...).
    pub normal: FxHashSet<String>,

    /// Imports used in dev targets (test, bench, example).
    pub dev: FxHashSet<String>,

    /// Imports used in build scripts.
    pub build: FxHashSet<String>,

    /// Mapping of imports to any relevant features.
    pub features: FxHashMap<String, Vec<FeatureRef>>,
}

impl CategorizedImports {
    /// All imports used in code (normal, dev, build).
    pub fn code_imports(&self) -> FxHashSet<String> {
        self.normal.union(&self.dev).chain(&self.build).cloned().collect()
    }

    /// All imports referenced only in features.
    pub fn feature_imports(&self) -> FxHashSet<String> {
        let code = self.code_imports();
        self.features.keys().filter(|key| !code.contains(*key)).cloned().collect()
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

            let imports: FxHashSet<String> = rust_files
                .par_iter()
                .map(|path| ParsedSource::from_path(path.as_path()))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flat_map(|parsed| parsed.imports)
                .collect();

            Self::categorize_imports(categorized, target_kind, imports);
        }

        Ok(())
    }

    fn analyze_with_expansion(
        categorized: &mut CategorizedImports,
        package: &Package,
    ) -> Result<()> {
        Self::analyze_from_files(categorized, package)?;

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

            let parsed = ParsedSource::from_str(&output_str);
            Self::categorize_imports(categorized, target_kind, parsed.imports);
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

    /// Collect import names for dependencies referenced in features.
    fn analyze_features(categorized: &mut CategorizedImports, manifest: &Manifest) {
        for (feature, values) in &manifest.features {
            for value in values {
                let (import, feature) = FeatureRef::parse(feature, value);
                categorized.features.entry(import).or_default().push(feature);
            }
        }

        // Collect implicit features from optional dependencies
        for (dep, details) in &manifest.dependencies {
            if details.get_ref().optional() {
                let import = dep.get_ref().replace('-', "_");
                let has_explicit = categorized.features.get(&import).is_some_and(|features| {
                    features.iter().any(|feature| matches!(feature, FeatureRef::Explicit { .. }))
                });

                if !has_explicit {
                    categorized.features.entry(import).or_default().push(FeatureRef::Implicit);
                }
            }
        }
    }
}
