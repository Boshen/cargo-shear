//! Package analysis for `cargo-shear`.

use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{Result, anyhow};
use cargo_metadata::TargetKind;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    context::PackageContext,
    manifest::{DepTable, FeatureRef},
    source_parser::ParsedSource,
};

/// Result of analyzing a package.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// Imports used in normal targets (lib, bin, ...).
    pub normal: FxHashSet<String>,

    /// Imports used in dev targets (test, bench, example).
    pub dev: FxHashSet<String>,

    /// Imports used in build scripts.
    pub build: FxHashSet<String>,

    /// Mapping of imports to any relevant features.
    pub features: FxHashMap<String, Vec<FeatureRef>>,

    /// Files that aren't reachable from any entry point.
    pub unlinked_files: FxHashSet<PathBuf>,

    /// Files that are empty (no items, only whitespace/comments).
    pub empty_files: FxHashSet<PathBuf>,
}

impl AnalysisResult {
    pub fn code_imports(&self) -> FxHashSet<String> {
        self.normal.union(&self.dev).chain(&self.build).cloned().collect()
    }

    pub fn feature_imports(&self) -> FxHashSet<String> {
        let code = self.code_imports();
        self.features.keys().filter(|key| !code.contains(*key)).cloned().collect()
    }
}

/// Analyzes a package to find unused dependencies and unlinked files.
///
/// The analyzer can operate in two modes:
/// - Normal: Parses source files directly (faster but may miss macro-generated code)
/// - Expand: Uses cargo expand to expand macros first (slower but more accurate)
pub struct PackageAnalyzer<'a> {
    /// Whether to use `cargo expand` to expand macros
    expand_macros: bool,

    /// Package context.
    ctx: &'a PackageContext<'a>,

    /// Accumulated analysis result.
    result: AnalysisResult,
}

impl<'a> PackageAnalyzer<'a> {
    pub fn new(ctx: &'a PackageContext<'a>, expand_macros: bool) -> Self {
        Self { expand_macros, ctx, result: AnalysisResult::default() }
    }

    pub fn analyze(mut self) -> Result<AnalysisResult> {
        self.analyze_from_files();

        if self.expand_macros {
            self.analyze_with_expansion()?;
        }

        self.analyze_features();
        self.analyze_unlinked();
        self.analyze_empty();

        Ok(self.result)
    }

    fn analyze_from_files(&mut self) {
        for target in &self.ctx.targets {
            let Some(kind) = target.kind.first() else {
                continue;
            };

            let is_build_script = target.kind.contains(&TargetKind::CustomBuild);
            let dir_bytes = if is_build_script {
                target.src_path.as_os_str().as_encoded_bytes()
            } else {
                target
                    .src_path
                    .parent()
                    .map_or(target.src_path.as_os_str(), |p| p.as_os_str())
                    .as_encoded_bytes()
            };

            let imports: FxHashSet<String> = self
                .ctx
                .workspace
                .files
                .iter()
                .filter(|(path, _)| {
                    let path = path.as_os_str().as_encoded_bytes();
                    if is_build_script {
                        // For build scripts, match the exact file
                        path == dir_bytes
                    } else {
                        // For directories, match files under that directory
                        path.starts_with(dir_bytes) && path.get(dir_bytes.len()) == Some(&b'/')
                    }
                })
                .flat_map(|(_, parsed)| parsed.imports.iter().cloned())
                .collect();

            match DepTable::from(kind) {
                DepTable::Normal => self.result.normal.extend(imports),
                DepTable::Dev => self.result.dev.extend(imports),
                DepTable::Build => self.result.build.extend(imports),
            }
        }
    }

    fn analyze_with_expansion(&mut self) -> Result<()> {
        for target in &self.ctx.targets {
            let Some(kind) = target.kind.first() else {
                continue;
            };

            let name = &target.name;
            let arg = match kind {
                TargetKind::CustomBuild => continue,
                TargetKind::Bin => format!("--bin={name}"),
                TargetKind::Example => format!("--example={name}"),
                TargetKind::Test => format!("--test={name}"),
                TargetKind::Bench => format!("--bench={name}"),
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
                .arg(&arg)
                .arg("--all-features")
                .arg("--profile=check")
                .arg("--")
                .arg("-Zunpretty=expanded")
                .current_dir(&self.ctx.directory)
                .stderr(Stdio::inherit());

            let output = cmd.output()?;
            if !output.status.success() {
                return Err(anyhow!("Cargo expand failed for {}", target.name));
            }

            let output = String::from_utf8(output.stdout)?;
            if output.is_empty() {
                return Err(anyhow!(
                    "Cargo expand failed for {}: Empty output from cargo expand",
                    target.name
                ));
            }

            let parsed = ParsedSource::from_expanded_str(&output, target.src_path.as_ref());
            match DepTable::from(kind) {
                DepTable::Normal => self.result.normal.extend(parsed.imports),
                DepTable::Dev => self.result.dev.extend(parsed.imports),
                DepTable::Build => self.result.build.extend(parsed.imports),
            }
        }

        Ok(())
    }

    fn analyze_features(&mut self) {
        for (feature, values) in &self.ctx.manifest.features {
            for value in values {
                let (import, feature) = FeatureRef::parse(feature, value);
                self.result.features.entry(import).or_default().push(feature);
            }
        }

        for (dep, details) in &self.ctx.manifest.dependencies {
            if details.get_ref().optional() {
                let import = dep.get_ref().replace('-', "_");
                let has_explicit = self.result.features.get(&import).is_some_and(|features| {
                    features.iter().any(|feature| matches!(feature, FeatureRef::Explicit { .. }))
                });

                if !has_explicit {
                    self.result.features.entry(import).or_default().push(FeatureRef::Implicit);
                }
            }
        }
    }

    fn analyze_unlinked(&mut self) {
        let dir_bytes = self.ctx.directory.as_os_str().as_encoded_bytes();
        self.result.unlinked_files = self
            .ctx
            .workspace
            .files
            .keys()
            .filter(|path| {
                let path_bytes = path.as_os_str().as_encoded_bytes();
                path_bytes.starts_with(dir_bytes)
                    && path_bytes.get(dir_bytes.len()) == Some(&b'/')
                    && !self.ctx.workspace.linked.contains(*path)
            })
            .cloned()
            .collect();
    }

    fn analyze_empty(&mut self) {
        let dir_bytes = self.ctx.directory.as_os_str().as_encoded_bytes();
        self.result.empty_files = self
            .ctx
            .workspace
            .files
            .iter()
            .filter(|(path, parsed)| {
                let path_bytes = path.as_os_str().as_encoded_bytes();
                // Only check files in this package that are linked (not entry points)
                path_bytes.starts_with(dir_bytes)
                    && path_bytes.get(dir_bytes.len()) == Some(&b'/')
                    && self.ctx.workspace.linked.contains(*path)
                    && parsed.is_empty
                    // Exclude entry points like lib.rs, main.rs, build.rs
                    && !self.ctx.targets.iter().any(|target| &target.src_path == *path)
            })
            .map(|(path, _)| path.clone())
            .collect();
    }
}
