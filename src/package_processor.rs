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
    env, fmt,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use cargo_metadata::{Metadata, NodeDep, Package};
use cargo_toml::{Dependency, DepsSet, Manifest};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::dependency_analyzer::{DependencyAnalyzer, FeatureRef};

/// Which table a dependency is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepTable {
    /// `[dependencies]`
    Normal,

    /// `[dev-dependencies]`
    Dev,

    /// `[build-dependencies]`
    Build,
}

impl fmt::Display for DepTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => f.write_str("dependencies"),
            Self::Dev => f.write_str("dev-dependencies"),
            Self::Build => f.write_str("build-dependencies"),
        }
    }
}

/// Location of a dependency in Cargo.toml.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepLocation {
    /// Package level dependency table.
    /// e.g. `[dependencies]`
    Root(DepTable),

    /// Target specific dependency table.
    /// e.g. `[target.cfg(unix).dependencies]`
    Target { cfg: String, table: DepTable },
}

impl DepLocation {
    const fn is_normal(&self) -> bool {
        matches!(self, Self::Root(DepTable::Normal) | Self::Target { table: DepTable::Normal, .. })
    }
}

impl fmt::Display for DepLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(table) => write!(f, "{table}"),
            Self::Target { cfg, table } => write!(f, "target.'{cfg}'.{table}"),
        }
    }
}

/// An unused dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedDependency {
    /// The dependency key.
    pub name: String,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,
}

/// An unused optional dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedOptionalDependency {
    /// The dependency key.
    pub name: String,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// An unused dependency only referenced in features.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedFeatureDependency {
    /// The dependency key.
    pub name: String,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// A misplaced dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedDependency {
    /// The dependency key.
    pub name: String,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,
}

/// A misplaced optional dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedOptionalDependency {
    /// The dependency key.
    pub name: String,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// An unused workspace dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedWorkspaceDependency {
    /// The dependency key.
    pub name: String,
}

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
    /// Used package names.
    pub used_packages: FxHashSet<String>,

    /// Unused dependencies.
    pub unused_dependencies: FxHashSet<UnusedDependency>,

    /// Unused optional dependencies.
    #[expect(dead_code, reason = "Tracked for future warnings")]
    pub unused_optional_dependencies: FxHashSet<UnusedOptionalDependency>,

    /// Unused dependencies only referenced in features.
    #[expect(dead_code, reason = "Tracked for future warnings")]
    pub unused_feature_dependencies: FxHashSet<UnusedFeatureDependency>,

    /// Misplaced dependencies.
    pub misplaced_dependencies: FxHashSet<MisplacedDependency>,

    /// Misplaced optional dependencies.
    #[expect(dead_code, reason = "Tracked for future warnings")]
    pub misplaced_optional_dependencies: FxHashSet<MisplacedOptionalDependency>,

    /// Redundant ignores.
    pub redundant_ignores: FxHashSet<String>,
}

/// Result of processing a workspace.
#[derive(Default)]
pub struct WorkspaceProcessResult {
    /// Unused workspace dependencies.
    pub unused_dependencies: FxHashSet<UnusedWorkspaceDependency>,

    /// Redundant workspace ignores.
    pub redundant_ignores: FxHashSet<String>,
}

impl PackageProcessor {
    /// Create a new package processor.
    pub const fn new(expand_macros: bool) -> Self {
        Self { analyzer: DependencyAnalyzer::new(expand_macros) }
    }

    /// Process a package to find unused/misplaced dependencies and track used packages.
    #[expect(clippy::too_many_lines, reason = "Main processing logic, not worth splitting up")]
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

        let import_to_pkg = Self::import_to_pkg_map(metadata, &resolved.deps)?;
        let pkg_to_import = Self::pkg_to_import_map(&import_to_pkg);
        let used_imports = self.analyzer.analyze_package(package, manifest)?;
        let all_used_imports = used_imports.all_imports();

        let ignored_imports: FxHashSet<String> = package_ignored_deps
            .iter()
            .chain(&workspace_ignored_deps)
            .map(|dep| dep.replace('-', "_"))
            .collect();

        let mut used_packages = FxHashSet::default();
        let mut unused_dependencies = FxHashSet::default();
        let mut unused_optional_dependencies = FxHashSet::default();
        let mut unused_feature_dependencies = FxHashSet::default();
        let mut misplaced_dependencies = FxHashSet::default();
        let mut misplaced_optional_dependencies = FxHashSet::default();

        for (&import, &pkg) in &import_to_pkg {
            if all_used_imports.contains(import) {
                used_packages.insert(pkg.to_owned());
            }
        }

        for (dep, dependency, location) in Self::all_dependencies(manifest) {
            let pkg = dependency
                .detail()
                .and_then(|d| d.package.as_ref())
                .map_or(dep.as_str(), String::as_str);

            let import = Self::resolve_import_name(&pkg_to_import, dep, pkg);
            if ignored_imports.contains(&*import) {
                continue;
            }

            let is_optional = dependency.optional();

            let used_in_normal = used_imports.normal.contains(&*import);
            let used_in_dev = used_imports.dev.contains(&*import);
            let used_in_build = used_imports.build.contains(&*import);

            let features = used_imports.features.get(&*import);
            let used_in_features = features.is_some();

            let used_in_code = used_in_normal || used_in_dev || used_in_build;
            if !used_in_code {
                if is_optional {
                    unused_optional_dependencies.insert(UnusedOptionalDependency {
                        name: dep.clone(),
                        location,
                        features: features.cloned().unwrap_or_default(),
                    });

                    continue;
                }

                if used_in_features {
                    unused_feature_dependencies.insert(UnusedFeatureDependency {
                        name: dep.clone(),
                        location,
                        features: features.cloned().unwrap_or_default(),
                    });
                } else {
                    unused_dependencies.insert(UnusedDependency { name: dep.clone(), location });
                }

                continue;
            }

            if location.is_normal() && !used_in_normal && used_in_dev {
                if is_optional {
                    misplaced_optional_dependencies.insert(MisplacedOptionalDependency {
                        name: dep.clone(),
                        location,
                        features: features.cloned().unwrap_or_default(),
                    });
                } else {
                    misplaced_dependencies
                        .insert(MisplacedDependency { name: dep.clone(), location });
                }
            }
        }

        let mut redundant_ignores = FxHashSet::default();
        for ignored_dep in &package_ignored_deps {
            let ignored_import = ignored_dep.replace('-', "_");

            let doesnt_exist = !import_to_pkg.contains_key(ignored_import.as_str());
            let is_used = all_used_imports.contains(&ignored_import);

            if doesnt_exist || is_used {
                redundant_ignores.insert((*ignored_dep).to_owned());
            }
        }

        Ok(PackageProcessResult {
            used_packages,
            unused_dependencies,
            unused_optional_dependencies,
            unused_feature_dependencies,
            misplaced_dependencies,
            misplaced_optional_dependencies,
            redundant_ignores,
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
                unused_dependencies.insert(UnusedWorkspaceDependency { name: dep.clone() });
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

    /// Build a map from package names to import names.
    fn pkg_to_import_map<'a>(
        import_to_pkg: &FxHashMap<&'a str, &'a str>,
    ) -> FxHashMap<&'a str, &'a str> {
        import_to_pkg.iter().map(|(&import, &pkg)| (pkg, import)).collect()
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

    /// Iterate over all dependencies in a manifest, including target specific ones.
    fn all_dependencies(manifest: &Manifest) -> Vec<(&String, &Dependency, DepLocation)> {
        let mut deps = Vec::new();

        for (dep, dependency) in &manifest.dependencies {
            deps.push((dep, dependency, DepLocation::Root(DepTable::Normal)));
        }

        for (dep, dependency) in &manifest.dev_dependencies {
            deps.push((dep, dependency, DepLocation::Root(DepTable::Dev)));
        }

        for (dep, dependency) in &manifest.build_dependencies {
            deps.push((dep, dependency, DepLocation::Root(DepTable::Build)));
        }

        for (cfg, target) in &manifest.target {
            for (dep, dependency) in &target.dependencies {
                deps.push((
                    dep,
                    dependency,
                    DepLocation::Target { cfg: cfg.clone(), table: DepTable::Normal },
                ));
            }

            for (dep, dependency) in &target.dev_dependencies {
                deps.push((
                    dep,
                    dependency,
                    DepLocation::Target { cfg: cfg.clone(), table: DepTable::Dev },
                ));
            }

            for (dep, dependency) in &target.build_dependencies {
                deps.push((
                    dep,
                    dependency,
                    DepLocation::Target { cfg: cfg.clone(), table: DepTable::Build },
                ));
            }
        }

        deps
    }
}
