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
    env,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use cargo_metadata::{Metadata, NodeDep, Package};
use cargo_toml::{Dependency, DepsSet, Manifest};
use cargo_util_schemas::core::PackageIdSpec;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::dependency_analyzer::DependencyAnalyzer;

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
#[derive(Default)]
pub struct PackageProcessResult {
    /// Unused dependency keys.
    pub unused_dependencies: FxHashSet<String>,

    /// Used package names.
    pub used_packages: FxHashSet<String>,

    /// Redundant ignores.
    pub redundant_ignores: FxHashSet<String>,

    /// Dependencies in [dependencies] that should be in [dev-dependencies].
    pub misplaced_dependencies: FxHashSet<String>,
}

/// Result of processing a workspace.
#[derive(Default)]
pub struct WorkspaceProcessResult {
    /// Unused workspace dependency keys.
    pub unused_dependencies: FxHashSet<String>,

    /// Redundant workspace ignores.
    pub redundant_ignores: FxHashSet<String>,
}

impl PackageProcessor {
    /// Create a new package processor.
    pub const fn new(expand_macros: bool) -> Self {
        Self { analyzer: DependencyAnalyzer::new(expand_macros) }
    }

    /// Process a package to find unused/misplaced dependencies and track used packages.
    pub fn process_package(
        &self,
        metadata: &Metadata,
        package: &Package,
        manifest: &Manifest,
    ) -> Result<PackageProcessResult> {
        let package_ignored_deps =
            DependencyAnalyzer::get_ignored_dependency_keys(&package.metadata);
        let workspace_ignored_deps =
            DependencyAnalyzer::get_ignored_dependency_keys(&metadata.workspace_metadata);

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

        let import_to_pkg = Self::import_to_pkg_map(&resolved.deps)?;
        let used_imports = self.analyzer.analyze_package(package, manifest)?;
        let all_used_imports = used_imports.all_imports();

        let ignored_imports: FxHashSet<String> = package_ignored_deps
            .iter()
            .chain(&workspace_ignored_deps)
            .map(|dep| dep.replace('-', "_"))
            .collect();

        let mut unused_dependencies = FxHashSet::default();
        let mut used_packages = FxHashSet::default();
        let mut misplaced_dependencies = FxHashSet::default();

        for (import, pkg) in &import_to_pkg {
            let dep = Self::find_dep(manifest, import);

            let is_used = all_used_imports.contains(import);
            if is_used {
                used_packages.insert(pkg.clone());
            }

            if ignored_imports.contains(import) {
                continue;
            }

            if !is_used {
                unused_dependencies.insert(dep);
                continue;
            }

            if !manifest.dependencies.contains_key(&dep) {
                continue;
            }

            let used_in_normal = used_imports.normal.contains(import);
            let used_in_dev = used_imports.dev.contains(import);
            let is_optional = manifest.dependencies.get(&dep).is_some_and(Dependency::optional);

            if !used_in_normal && used_in_dev && !is_optional {
                misplaced_dependencies.insert(dep);
            }
        }

        let mut redundant_ignores = FxHashSet::default();
        for ignored_dep in &package_ignored_deps {
            let ignored_import = ignored_dep.replace('-', "_");

            let doesnt_exist = !import_to_pkg.contains_key(&ignored_import);
            let is_used = all_used_imports.contains(&ignored_import);

            if doesnt_exist || is_used {
                redundant_ignores.insert((*ignored_dep).to_owned());
            }
        }

        Ok(PackageProcessResult {
            unused_dependencies,
            used_packages,
            redundant_ignores,
            misplaced_dependencies,
        })
    }

    /// Process workspace to find unused workspace dependencies.
    pub fn process_workspace(
        manifest: &Manifest,
        metadata: &Metadata,
        workspace_used_pkgs: &FxHashSet<String>,
    ) -> WorkspaceProcessResult {
        if metadata.workspace_packages().len() <= 1 {
            return WorkspaceProcessResult::default();
        }

        let Some(workspace) = &manifest.workspace else {
            return WorkspaceProcessResult::default();
        };

        let ignored_deps =
            DependencyAnalyzer::get_ignored_dependency_keys(&metadata.workspace_metadata);

        let dep_to_pkg = Self::dep_to_pkg_map(&workspace.dependencies);

        let mut unused_dependencies = FxHashSet::default();
        for (dep, pkg) in &dep_to_pkg {
            if ignored_deps.contains(dep.as_str()) {
                continue;
            }

            if !workspace_used_pkgs.contains(pkg) {
                unused_dependencies.insert(dep.clone());
            }
        }

        let mut redundant_ignores = FxHashSet::default();
        for ignored_dep in &ignored_deps {
            let doesnt_exist = !dep_to_pkg.contains_key(*ignored_dep);
            let is_used =
                dep_to_pkg.get(*ignored_dep).is_some_and(|pkg| workspace_used_pkgs.contains(pkg));

            if doesnt_exist || is_used {
                redundant_ignores.insert((*ignored_dep).to_owned());
            }
        }

        WorkspaceProcessResult { unused_dependencies, redundant_ignores }
    }

    /// Get the relative path for a manifest, preferring current dir over workspace root.
    pub fn get_relative_path(manifest_path: &Path, workspace_root: &Path) -> PathBuf {
        let dir = manifest_path.parent().unwrap_or(manifest_path);

        let current_dir = env::current_dir().unwrap_or_default();

        manifest_path
            .strip_prefix(&current_dir)
            .or_else(|_| manifest_path.strip_prefix(workspace_root))
            .unwrap_or(dir)
            .to_path_buf()
    }

    /// Build a map from import names to package names from resolved dependencies.
    fn import_to_pkg_map(imports: &[NodeDep]) -> Result<FxHashMap<String, String>> {
        imports
            .iter()
            .map(|import| {
                Self::parse_package_id(&import.pkg.repr).map(|pkg| (import.name.clone(), pkg))
            })
            .collect()
    }

    /// Build a map from dependency keys to package names from a `DepsSet`.
    fn dep_to_pkg_map(deps: &DepsSet) -> FxHashMap<String, String> {
        deps.iter()
            .map(|(dep, dependency)| {
                let pkg = dependency
                    .detail()
                    .and_then(|detail| detail.package.as_ref())
                    .map_or_else(|| dep.to_owned(), Clone::clone);

                (dep.clone(), pkg)
            })
            .collect()
    }

    /// Find the dependency key for an import name, checking if it exists in the manifest.
    fn find_dep(manifest: &Manifest, import: &str) -> String {
        // Look for either a hyphen or underscore version of the import.
        let dep = import.replace('_', "-");

        let exists = manifest.dependencies.contains_key(&dep)
            || manifest.dev_dependencies.contains_key(&dep)
            || manifest.build_dependencies.contains_key(&dep)
            || manifest.target.values().any(|target| target.dependencies.contains_key(&dep));

        if exists { dep } else { import.to_owned() }
    }

    /// Parse a package ID string to extract the package name.
    ///
    /// Package IDs can have different formats depending on the Rust version:
    /// - Pre-1.77: `memchr 2.7.1 (registry+https://github.com/rust-lang/crates.io-index)`
    /// - 1.77+: `registry+https://github.com/rust-lang/crates.io-index#memchr@2.7.1`
    fn parse_package_id(repr: &str) -> Result<String> {
        if repr.contains(' ') {
            repr.split(' ')
                .next()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Parse error: {repr} should have a space"))
        } else {
            PackageIdSpec::parse(repr)
                .map(|id| id.name().to_owned())
                .map_err(|e| anyhow!("Parse error: {e}"))
        }
    }
}
