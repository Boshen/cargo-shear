//! Per-package import collection and source-file inventory.

use std::{
    env,
    ffi::OsString,
    path::PathBuf,
    process::{Command, Stdio},
};

use anyhow::{Result, anyhow};
use cargo_metadata::TargetKind;
use compact_str::CompactString;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    context::PackageContext,
    manifest::{DepTable, FeatureRef, lib_kind_label},
    source_parser::ParsedSource,
};

/// Configured `test`/`doctest` flags for one lib-like target alongside what its source actually contains.
#[derive(Debug)]
pub struct TargetTestInfo {
    /// Cargo target name.
    pub target_name: String,
    /// Target kind from `cargo_metadata` (lib, rlib, proc-macro, ...).
    pub target_kind: TargetKind,
    /// `test = true` in `Cargo.toml` (or absent, since `true` is the default).
    pub test_enabled: bool,
    /// `doctest = true` in `Cargo.toml` (or absent, since `true` is the default).
    pub doctest_enabled: bool,
    /// At least one source file in this target has `#[test]` or `#[cfg(test)]`.
    pub has_tests: bool,
    /// At least one source file has executable doc tests (Rust code blocks, non-`ignore`).
    pub has_doctests: bool,
}

/// Everything `PackageAnalyzer` accumulates for one package: imports bucketed by target
/// kind, plus the file-level findings used for unlinked/empty diagnostics.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// Imports observed in normal targets (lib, bin, ...).
    pub normal: FxHashSet<CompactString>,

    /// Imports observed in dev targets (test, bench, example).
    pub dev: FxHashSet<CompactString>,

    /// Imports observed in build scripts.
    pub build: FxHashSet<CompactString>,

    /// Imports referenced from `[features]` entries, with the feature entries that name them.
    pub features: FxHashMap<CompactString, Vec<FeatureRef>>,

    /// Source files not reachable from any entry point (lib.rs, main.rs, ...).
    pub unlinked_files: FxHashSet<PathBuf>,

    /// Reachable source files that contain no items (only whitespace/comments).
    pub empty_files: FxHashSet<PathBuf>,

    /// One entry per lib-like target: configured `test`/`doctest` flags vs. observed contents.
    pub target_test_info: Vec<TargetTestInfo>,
}

impl AnalysisResult {
    pub fn code_imports(&self) -> FxHashSet<CompactString> {
        self.normal.union(&self.dev).chain(&self.build).cloned().collect()
    }

    /// Imports referenced only from `[features]`, excluding any that already
    /// appear in code. `code_imports` is taken as a parameter so callers that
    /// already computed it (via [`code_imports`](Self::code_imports)) don't pay
    /// to rebuild the union set.
    pub fn feature_imports(
        &self,
        code_imports: &FxHashSet<CompactString>,
    ) -> FxHashSet<CompactString> {
        self.features.keys().filter(|key| !code_imports.contains(key.as_str())).cloned().collect()
    }
}

/// Walks a package's targets and source files to populate an [`AnalysisResult`].
///
/// Two modes:
/// - Normal: parses source files directly. Fast, but misses imports introduced
///   by macro expansion.
/// - Expand: shells out to `cargo expand` first, then parses the expanded output.
///   More accurate but significantly slower and nightly-only.
pub struct PackageAnalyzer<'a> {
    /// Use `cargo expand` to expand macros before parsing.
    expand_macros: bool,

    /// Workspace + package context this analyzer reads from.
    ctx: &'a PackageContext<'a>,

    /// Findings collected so far; returned from [`analyze`](Self::analyze).
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

            let matching_files: Vec<_> = self
                .ctx
                .workspace
                .files
                .iter()
                .filter(|(path, _)| {
                    let path = path.as_os_str().as_encoded_bytes();
                    if is_build_script {
                        path == dir_bytes
                    } else {
                        path.starts_with(dir_bytes)
                            && matches!(path.get(dir_bytes.len()), Some(&(b'/' | b'\\')))
                    }
                })
                .collect();

            let imports: FxHashSet<CompactString> = matching_files
                .iter()
                .flat_map(|(_, parsed)| parsed.imports.iter().cloned())
                .collect();

            match DepTable::from(kind) {
                DepTable::Normal => self.result.normal.extend(imports),
                DepTable::Dev => self.result.dev.extend(imports),
                DepTable::Build => self.result.build.extend(imports),
            }

            // Lib-like targets only. Bin targets share source directories with the
            // package's lib target, so we can't tell which file's tests "belong" to
            // which target — recording for both would produce duplicate diagnostics.
            if lib_kind_label(kind).is_some() {
                let has_tests = matching_files.iter().any(|(_, parsed)| parsed.has_tests);
                let has_doctests = matching_files.iter().any(|(_, parsed)| parsed.has_doctests);

                self.result.target_test_info.push(TargetTestInfo {
                    target_name: target.name.clone(),
                    target_kind: kind.clone(),
                    test_enabled: target.test,
                    doctest_enabled: target.doctest,
                    has_tests,
                    has_doctests,
                });
            }
        }
    }

    fn analyze_with_expansion(&mut self) -> Result<()> {
        for target in &self.ctx.targets {
            let Some(kind) = target.kind.first() else {
                continue;
            };

            let name = &target.name;
            // `--profile=test` makes cargo include dev-dependencies and pass `--test` to
            // rustc, so `#[cfg(test)]` blocks survive expansion and dev-deps used only
            // through macros inside them are visible. `--test=` / `--bench=` already
            // build in test mode, so they keep the cheaper `--profile=check`.
            let (arg, profile) = match kind {
                TargetKind::CustomBuild => continue,
                TargetKind::Bin => (format!("--bin={name}"), "--profile=test"),
                TargetKind::Example => (format!("--example={name}"), "--profile=test"),
                TargetKind::Test => (format!("--test={name}"), "--profile=check"),
                TargetKind::Bench => (format!("--bench={name}"), "--profile=check"),
                TargetKind::CDyLib
                | TargetKind::DyLib
                | TargetKind::Lib
                | TargetKind::ProcMacro
                | TargetKind::RLib
                | TargetKind::StaticLib
                | TargetKind::Unknown(_)
                | _ => ("--lib".to_owned(), "--profile=test"),
            };

            let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

            let mut cmd = Command::new(cargo);
            cmd.arg("rustc")
                .arg(&arg)
                .arg("--all-features")
                .arg(profile)
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

            let parsed = ParsedSource::from_str(&output, target.src_path.as_ref());
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
                self.result.features.entry(import.into()).or_default().push(feature);
            }
        }

        for (dep, details) in &self.ctx.manifest.dependencies {
            if details.get_ref().optional() {
                let import = CompactString::from(dep.get_ref().replace('-', "_"));
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
                // The workspace root walks every Rust file in the tree, including those
                // owned by member packages. Don't double-count their files here.
                if self.ctx.directory == self.ctx.workspace.root
                    && self.ctx.workspace.packages.len() > 1
                {
                    for pkg in &self.ctx.workspace.packages {
                        if pkg != &self.ctx.directory && path.starts_with(pkg) {
                            return false;
                        }
                    }
                }

                let path_bytes = path.as_os_str().as_encoded_bytes();
                path_bytes.starts_with(dir_bytes)
                    && matches!(path_bytes.get(dir_bytes.len()), Some(&(b'/' | b'\\')))
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
                // Restrict to files inside this package that are reachable from a module tree.
                path_bytes.starts_with(dir_bytes)
                    && matches!(path_bytes.get(dir_bytes.len()), Some(&(b'/' | b'\\')))
                    && self.ctx.workspace.linked.contains(*path)
                    && parsed.is_empty
                    // Entry points (lib.rs, main.rs, build.rs, ...) are routinely
                    // empty when scaffolding a new target — never warn on those.
                    && !self.ctx.targets.iter().any(|target| &target.src_path == *path)
            })
            .map(|(path, _)| path.clone())
            .collect();
    }
}
