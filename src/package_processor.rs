//! Package processing module for cargo-shear.
//!
//! This module coordinates the analysis of individual packages and workspaces
//! to identify unused dependencies. It combines dependency metadata from
//! cargo with the actual usage analysis to determine which dependencies
//! can be safely removed.
//!
//! # Terminology
//!
//! * import: Imports from within Rust code:
//!
//! ```rust
//! use tokio_util::codec;
//! ```
//!
//! Here: `tokio_util`
//!
//! * dep: Dependency keys from `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tokio-util = "0.7"
//! ```
//!
//! Here: `tokio-util`
//!
//! * pkg: Package names from the registry:
//!
//! ```toml
//! [dependencies]
//! pki-types = { package = "rustls-pki-types", version = "1.12" }
//! ```
//!
//! Here: `rustls-pki-types`

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use cargo_metadata::{Metadata, NodeDep, Package};
use miette::NamedSource;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    dependency_analyzer::DependencyAnalyzer,
    diagnostic::{
        BoxedDiagnostic, MisplacedDependency, MisplacedOptionalDependency, RedundantIgnore,
        RelatedAdvice, UnusedDependency, UnusedOptionalDependency, UnusedWorkspaceDependency,
    },
    manifest::{DepLocation, DepTable, DepsSet, Manifest},
};

/// Processes packages to identify unused dependencies.
///
/// The processor uses a `DependencyAnalyzer` to determine which dependencies
/// are actually used, then compares with the declared dependencies to find
/// unused ones.
pub struct PackageProcessor {
    /// The analyzer used to find used dependencies in source code
    analyzer: DependencyAnalyzer,
}

/// Result of processing a package.
pub struct PackageProcessResult {
    /// Path to the manifest file.
    pub manifest: PathBuf,

    /// Used package names.
    pub used_packages: FxHashSet<String>,

    /// Unused dependency diagnostics.
    pub unused: Vec<UnusedDependency>,

    /// Unused optional dependency diagnostics.
    pub unused_optional: Vec<UnusedOptionalDependency>,

    /// Misplaced dependency diagnostics.
    pub misplaced: Vec<MisplacedDependency>,

    /// Misplaced optional dependency diagnostics.
    pub misplaced_optional: Vec<MisplacedOptionalDependency>,

    /// Redundant ignore diagnostics.
    pub redundant_ignores: Vec<RedundantIgnore>,
}

impl PackageProcessResult {
    #[must_use]
    fn new(manifest: &Path) -> Self {
        Self {
            manifest: manifest.to_path_buf(),
            used_packages: FxHashSet::default(),
            unused: Vec::new(),
            unused_optional: Vec::new(),
            misplaced: Vec::new(),
            misplaced_optional: Vec::new(),
            redundant_ignores: Vec::new(),
        }
    }

    /// Check if there are any issues to report.
    #[must_use]
    pub const fn has_issues(&self) -> bool {
        !self.unused.is_empty()
            || !self.unused_optional.is_empty()
            || !self.misplaced.is_empty()
            || !self.misplaced_optional.is_empty()
            || !self.redundant_ignores.is_empty()
    }

    /// Check if there are any fixable issues.
    #[must_use]
    pub const fn has_fixable_issues(&self) -> bool {
        !self.unused.is_empty() || !self.misplaced.is_empty()
    }

    /// Collect all diagnostics from this result.
    #[must_use]
    pub fn diagnostics(&mut self) -> Vec<BoxedDiagnostic> {
        let mut diagnostics: Vec<BoxedDiagnostic> = Vec::new();

        self.unused.sort_by_key(|d| d.span.0);
        for diag in self.unused.drain(..) {
            diagnostics.push(Box::new(diag));
        }
        self.unused_optional.sort_by_key(|d| d.span.0);
        for diag in self.unused_optional.drain(..) {
            diagnostics.push(Box::new(diag));
        }

        self.misplaced.sort_by_key(|d| d.span.0);
        for diag in self.misplaced.drain(..) {
            diagnostics.push(Box::new(diag));
        }
        self.misplaced_optional.sort_by_key(|d| d.span.0);
        for diag in self.misplaced_optional.drain(..) {
            diagnostics.push(Box::new(diag));
        }

        self.redundant_ignores.sort_by_key(|d| d.span.0);
        for diag in self.redundant_ignores.drain(..) {
            diagnostics.push(Box::new(diag));
        }

        diagnostics
    }
}

/// Result of processing a workspace.
pub struct WorkspaceProcessResult {
    /// Path to the manifest file.
    pub manifest: PathBuf,

    /// Unused workspace dependency diagnostics.
    pub unused: Vec<UnusedWorkspaceDependency>,

    /// Redundant ignore diagnostics.
    pub redundant_ignores: Vec<RedundantIgnore>,
}

impl WorkspaceProcessResult {
    #[must_use]
    fn new(manifest: &Path) -> Self {
        Self { manifest: manifest.to_path_buf(), unused: Vec::new(), redundant_ignores: Vec::new() }
    }

    /// Check if there are any issues to report.
    #[must_use]
    pub const fn has_issues(&self) -> bool {
        !self.unused.is_empty() || !self.redundant_ignores.is_empty()
    }

    /// Check if there are any fixable issues.
    #[must_use]
    pub const fn has_fixable_issues(&self) -> bool {
        !self.unused.is_empty()
    }

    /// Collect all diagnostics from this result.
    #[must_use]
    pub fn diagnostics(&mut self) -> Vec<BoxedDiagnostic> {
        let mut diagnostics: Vec<BoxedDiagnostic> = Vec::new();

        self.unused.sort_by_key(|d| d.span.0);
        for diag in self.unused.drain(..) {
            diagnostics.push(Box::new(diag));
        }

        self.redundant_ignores.sort_by_key(|d| d.span.0);
        for diag in self.redundant_ignores.drain(..) {
            diagnostics.push(Box::new(diag));
        }

        diagnostics
    }
}

impl PackageProcessor {
    /// Create a new package processor.
    pub const fn new(expand_macros: bool) -> Self {
        Self { analyzer: DependencyAnalyzer::new(expand_macros) }
    }

    /// Process a package to find unused/misplaced dependencies and track used packages.
    #[expect(clippy::too_many_lines, reason = "TODO")]
    pub fn process_package(
        &self,
        metadata: &Metadata,
        package: &Package,
        manifest: &Manifest,
        manifest_content: &str,
        workspace_manifest: &Manifest,
        workspace_root: &Path,
    ) -> Result<PackageProcessResult> {
        let package_ignored = &manifest.package.metadata.cargo_shear.ignored;
        let workspace_ignored = &workspace_manifest.workspace.metadata.cargo_shear.ignored;

        let path = package.manifest_path.as_std_path();
        let relative = path
            .strip_prefix(workspace_root)
            .map_or_else(|_| path.to_path_buf(), Path::to_path_buf);

        let src = NamedSource::new(relative.display().to_string(), manifest_content.to_owned());

        let resolved = metadata
            .resolve
            .as_ref()
            .ok_or_else(|| {
                anyhow!("`cargo_metadata::MetadataCommand::no_deps` should not be called.")
            })?
            .nodes
            .iter()
            .find(|node| node.id == package.id)
            .ok_or_else(|| anyhow!("Package not found: {}", package.name))?;

        let import_to_pkg = Self::import_to_pkg_map(metadata, &resolved.deps)?;
        let pkg_to_import = Self::pkg_to_import_map(&import_to_pkg);
        let used_imports = self.analyzer.analyze_package(package, manifest)?;
        let all_used_imports = used_imports.all_imports();

        let ignored_imports: FxHashSet<String> = package_ignored
            .iter()
            .chain(workspace_ignored.iter())
            .map(|dep| dep.get_ref().replace('-', "_"))
            .collect();

        let mut result = PackageProcessResult::new(package.manifest_path.as_std_path());

        for (import, pkg) in &import_to_pkg {
            if all_used_imports.contains(*import) {
                result.used_packages.insert((*pkg).to_owned());
            }
        }

        for (dep, details, location) in manifest.all_dependencies() {
            let span = dep.span();
            let dep_name = dep.get_ref();
            let pkg = details.get_ref().package().unwrap_or(dep_name);
            let import = Self::resolve_import_name(&pkg_to_import, dep_name, pkg);

            let is_optional = details.get_ref().optional();

            if ignored_imports.contains(&*import) {
                continue;
            }

            let used_in_normal = used_imports.normal.contains(&*import);
            let used_in_dev = used_imports.dev.contains(&*import);
            let used_in_build = used_imports.build.contains(&*import);
            let used_in_features = used_imports.features.contains_key(&*import);

            let used_in_code = used_in_normal || used_in_dev || used_in_build;
            if !used_in_code {
                if is_optional {
                    let features = used_imports.features.get(&*import).cloned().unwrap_or_default();
                    let mut related: Vec<RelatedAdvice> = features
                        .into_iter()
                        .map(|f| RelatedAdvice::UsedInFeature {
                            src: src.clone(),
                            span: (f.span.start, f.span.end - f.span.start),
                            feature: f.name,
                        })
                        .collect();
                    related.push(RelatedAdvice::BreakingChange);
                    result.unused_optional.push(UnusedOptionalDependency {
                        src: src.clone(),
                        span: (span.start, span.end - span.start),
                        name: dep_name.clone(),
                        related,
                    });
                    continue;
                }

                if !used_in_features {
                    result.unused.push(UnusedDependency {
                        src: src.clone(),
                        span: (span.start, span.end - span.start),
                        name: dep_name.clone(),
                    });
                }

                continue;
            }

            let is_dependencies_table = matches!(
                location,
                DepLocation::Root(DepTable::Normal)
                    | DepLocation::Target { table: DepTable::Normal, .. }
            );

            if is_dependencies_table && !used_in_normal && used_in_dev {
                if is_optional {
                    let target = location.as_table(DepTable::Dev);
                    let features = used_imports.features.get(&*import).cloned().unwrap_or_default();
                    let mut related: Vec<RelatedAdvice> = features
                        .into_iter()
                        .map(|f| RelatedAdvice::UsedInFeature {
                            src: src.clone(),
                            span: (f.span.start, f.span.end - f.span.start),
                            feature: f.name,
                        })
                        .collect();
                    related.push(RelatedAdvice::BreakingChange);
                    result.misplaced_optional.push(MisplacedOptionalDependency {
                        src: src.clone(),
                        span: (span.start, span.end - span.start),
                        help: format!(
                            "consider removing the `optional` flag and moving to `{target}`, or suppressing this warning"
                        ),
                        name: dep_name.clone(),
                        related,
                    });
                    continue;
                }

                let target = location.as_table(DepTable::Dev);
                result.misplaced.push(MisplacedDependency {
                    src: src.clone(),
                    span: (span.start, span.end - span.start),
                    help: format!("move this dependency to `{target}`"),
                    name: dep_name.clone(),
                    location: location.clone(),
                });
            }
        }

        // Check package-level ignored dependencies
        for dep in package_ignored {
            let span = dep.span();
            let dep = dep.get_ref();

            let ignored_import = dep.replace('-', "_");

            let doesnt_exist = !import_to_pkg.contains_key(ignored_import.as_str());
            let is_used = all_used_imports.contains(&ignored_import);

            if doesnt_exist || is_used {
                let reason = if doesnt_exist {
                    "not a declared dependency".to_owned()
                } else {
                    "dependency is used".to_owned()
                };
                result.redundant_ignores.push(RedundantIgnore {
                    src: src.clone(),
                    span: (span.start, span.end - span.start),
                    name: dep.clone(),
                    reason,
                });
            }
        }

        Ok(result)
    }

    /// Process workspace to find unused workspace dependencies.
    pub fn process_workspace(
        path: &Path,
        manifest: &Manifest,
        manifest_content: &str,
        metadata: &Metadata,
        workspace_used_pkgs: &FxHashSet<String>,
        workspace_root: &Path,
    ) -> WorkspaceProcessResult {
        if metadata.workspace_packages().len() <= 1 {
            return WorkspaceProcessResult::new(path);
        }

        let relative = path
            .strip_prefix(workspace_root)
            .map_or_else(|_| path.to_path_buf(), Path::to_path_buf);
        let src = NamedSource::new(relative.display().to_string(), manifest_content.to_owned());

        let dep_to_pkg = Self::dep_to_pkg_map(&manifest.workspace.dependencies);
        let workspace_ignored = &manifest.workspace.metadata.cargo_shear.ignored;

        let ignored_dep_keys: FxHashSet<&str> =
            workspace_ignored.iter().map(|s| s.get_ref().as_str()).collect();

        let mut result = WorkspaceProcessResult::new(path);

        for dep in manifest.workspace.dependencies.keys() {
            let span = dep.span();
            let dep_name = dep.get_ref();

            if ignored_dep_keys.contains(dep_name.as_str()) {
                continue;
            }

            let pkg = dep_to_pkg[dep_name.as_str()];
            if !workspace_used_pkgs.contains(pkg) {
                result.unused.push(UnusedWorkspaceDependency {
                    src: src.clone(),
                    span: (span.start, span.end - span.start),
                    name: dep_name.clone(),
                });
            }
        }

        // Check workspace-level ignored dependencies
        for dep in workspace_ignored {
            let span = dep.span();
            let dep = dep.get_ref();

            let doesnt_exist = !dep_to_pkg.contains_key(dep.as_str());
            let is_used =
                dep_to_pkg.get(dep.as_str()).is_some_and(|&pkg| workspace_used_pkgs.contains(pkg));

            if doesnt_exist || is_used {
                let reason = if doesnt_exist {
                    "not a declared dependency".to_owned()
                } else {
                    "dependency is used".to_owned()
                };
                result.redundant_ignores.push(RedundantIgnore {
                    src: src.clone(),
                    span: (span.start, span.end - span.start),
                    name: dep.clone(),
                    reason,
                });
            }
        }

        result
    }

    /// Build a map from import names to package names from resolved dependencies.
    fn import_to_pkg_map<'a>(
        metadata: &'a Metadata,
        imports: &'a [NodeDep],
    ) -> Result<FxHashMap<&'a str, &'a str>> {
        imports
            .iter()
            .map(|import| {
                let pkg = metadata
                    .packages
                    .iter()
                    .find(|p| p.id == import.pkg)
                    .ok_or_else(|| anyhow!("Package not found: {}", import.pkg.repr))?;

                Ok((import.name.as_str(), pkg.name.as_str()))
            })
            .collect()
    }

    /// Build a map from dependency keys to package names from a `DepsSet`.
    fn dep_to_pkg_map(deps: &DepsSet) -> FxHashMap<&str, &str> {
        deps.iter()
            .map(|(dep, details)| {
                let dep = dep.get_ref();
                let pkg = details.get_ref().package().unwrap_or(dep);
                (dep.as_str(), pkg)
            })
            .collect()
    }

    /// Build a reverse map from package names to import names.
    fn pkg_to_import_map<'a>(
        import_to_pkg: &FxHashMap<&'a str, &'a str>,
    ) -> FxHashMap<&'a str, &'a str> {
        import_to_pkg.iter().map(|(&import, &pkg)| (pkg, import)).collect()
    }

    /// Resolve the import name for a dependency.
    fn resolve_import_name<'a>(
        pkg_to_import: &'a FxHashMap<&str, &str>,
        dep: &'a str,
        pkg: &str,
    ) -> Cow<'a, str> {
        pkg_to_import
            .get(pkg)
            .map_or_else(|| Cow::Owned(dep.replace('-', "_")), |&import| Cow::Borrowed(import))
    }
}
