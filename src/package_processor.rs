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

use std::borrow::Cow;

use anyhow::{Result, anyhow};
use cargo_metadata::{Metadata, NodeDep, Package};
use rustc_hash::{FxHashMap, FxHashSet};
use toml::Spanned;

use crate::{
    dependency_analyzer::{DependencyAnalyzer, FeatureRef},
    manifest::{DepLocation, DepsSet, Manifest},
};

/// An unused dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedDependency {
    /// The dependency key.
    pub name: Spanned<String>,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,
}

/// An unused optional dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedOptionalDependency {
    /// The dependency key.
    pub name: Spanned<String>,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// An unused dependency only referenced in features.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedFeatureDependency {
    /// The dependency key.
    pub name: Spanned<String>,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// An unused workspace dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedWorkspaceDependency {
    /// The dependency key.
    pub name: Spanned<String>,
}

/// A misplaced dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedDependency {
    /// The dependency key.
    pub name: Spanned<String>,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,
}

/// A misplaced optional dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedOptionalDependency {
    /// The dependency key.
    pub name: Spanned<String>,

    /// Where the dependency is in the manifest.
    pub location: DepLocation,

    /// Features referencing this dependency.
    pub features: Vec<FeatureRef>,
}

/// An unknown ignore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownIgnore {
    /// The dependency key.
    pub name: Spanned<String>,
}

/// An redundant ignore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RedundantIgnore {
    /// The dependency key.
    pub name: Spanned<String>,
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
pub struct PackageAnalysis {
    /// Used package names.
    pub used_packages: FxHashSet<String>,

    /// Unused dependencies.
    pub unused_dependencies: Vec<UnusedDependency>,

    /// Unused optional dependencies.
    pub unused_optional_dependencies: Vec<UnusedOptionalDependency>,

    /// Unused dependencies only referenced in features.
    pub unused_feature_dependencies: Vec<UnusedFeatureDependency>,

    /// Misplaced dependencies.
    pub misplaced_dependencies: Vec<MisplacedDependency>,

    /// Misplaced optional dependencies.
    pub misplaced_optional_dependencies: Vec<MisplacedOptionalDependency>,

    /// Unknown ignores.
    pub unknown_ignores: Vec<UnknownIgnore>,

    /// Redundant ignores.
    pub redundant_ignores: Vec<RedundantIgnore>,
}

/// Result of processing a workspace.
#[derive(Default)]
pub struct WorkspaceAnalysis {
    /// Unused workspace dependencies.
    pub unused_dependencies: Vec<UnusedWorkspaceDependency>,

    /// Unknown workspace ignores.
    pub unknown_ignores: Vec<UnknownIgnore>,

    /// Redundant workspace ignores.
    pub redundant_ignores: Vec<RedundantIgnore>,
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
        workspace_manifest: &Manifest,
    ) -> Result<PackageAnalysis> {
        let mut result = PackageAnalysis::default();

        let package_ignored_deps = &manifest.package.metadata.cargo_shear.ignored;
        let workspace_ignored_deps = &workspace_manifest.workspace.metadata.cargo_shear.ignored;

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
        let code_imports = used_imports.code_imports();
        let feature_imports = used_imports.feature_imports();

        let ignored_imports: FxHashSet<String> = package_ignored_deps
            .iter()
            .chain(workspace_ignored_deps)
            .map(|dep| dep.get_ref().replace('-', "_"))
            .collect();

        for (&import, &pkg) in &import_to_pkg {
            if code_imports.contains(import) || feature_imports.contains(import) {
                result.used_packages.insert(pkg.to_owned());
            }
        }

        for (dep, dependency, location) in manifest.all_dependencies() {
            let pkg = dependency.get_ref().package().unwrap_or_else(|| dep.get_ref().as_str());

            let import = Self::resolve_import_name(&pkg_to_import, dep.get_ref(), pkg);
            if ignored_imports.contains(&*import) {
                continue;
            }

            if !code_imports.contains(&*import) {
                if dependency.get_ref().optional() {
                    result.unused_optional_dependencies.push(UnusedOptionalDependency {
                        name: dep.clone(),
                        features: used_imports.features.get(&*import).cloned().unwrap_or_default(),
                    });

                    continue;
                }

                if feature_imports.contains(&*import) {
                    result.unused_feature_dependencies.push(UnusedFeatureDependency {
                        name: dep.clone(),
                        features: used_imports.features.get(&*import).cloned().unwrap_or_default(),
                    });

                    continue;
                }

                result
                    .unused_dependencies
                    .push(UnusedDependency { name: dep.clone(), location: location.clone() });

                continue;
            }

            if location.is_normal()
                && !used_imports.normal.contains(&*import)
                && used_imports.dev.contains(&*import)
            {
                if dependency.get_ref().optional() {
                    result.misplaced_optional_dependencies.push(MisplacedOptionalDependency {
                        name: dep.clone(),
                        location: location.clone(),
                        features: used_imports.features.get(&*import).cloned().unwrap_or_default(),
                    });
                } else {
                    result.misplaced_dependencies.push(MisplacedDependency {
                        name: dep.clone(),
                        location: location.clone(),
                    });
                }
            }
        }

        for ignored_dep in package_ignored_deps {
            let ignored_import = ignored_dep.get_ref().replace('-', "_");

            if !import_to_pkg.contains_key(ignored_import.as_str()) {
                result.unknown_ignores.push(UnknownIgnore { name: ignored_dep.clone() });
                continue;
            }

            if code_imports.contains(&ignored_import) {
                result.redundant_ignores.push(RedundantIgnore { name: ignored_dep.clone() });
            }
        }

        Ok(result)
    }

    /// Process workspace to find unused workspace dependencies.
    pub fn process_workspace(
        manifest: &Manifest,
        metadata: &Metadata,
        workspace_used_pkgs: &FxHashSet<String>,
    ) -> WorkspaceAnalysis {
        let mut result = WorkspaceAnalysis::default();

        if metadata.workspace_packages().len() <= 1 {
            return result;
        }

        if manifest.workspace.dependencies.is_empty() {
            return result;
        }

        let ignored_deps = &manifest.workspace.metadata.cargo_shear.ignored;
        let ignored_dep_keys: FxHashSet<&str> =
            ignored_deps.iter().map(|s| s.get_ref().as_str()).collect();

        for (dep, dependency) in &manifest.workspace.dependencies {
            if ignored_dep_keys.contains(dep.get_ref().as_str()) {
                continue;
            }

            let pkg =
                dependency.get_ref().package().map_or_else(|| dep.get_ref().clone(), str::to_owned);

            if !workspace_used_pkgs.contains(&pkg) {
                result.unused_dependencies.push(UnusedWorkspaceDependency { name: dep.clone() });
            }
        }

        let dep_to_pkg = Self::dep_to_pkg_map(&manifest.workspace.dependencies);
        for ignored_dep in ignored_deps {
            if !dep_to_pkg.contains_key(ignored_dep.get_ref()) {
                result.unknown_ignores.push(UnknownIgnore { name: ignored_dep.clone() });
                continue;
            }

            if dep_to_pkg
                .get(ignored_dep.get_ref())
                .is_some_and(|pkg| workspace_used_pkgs.contains(pkg))
            {
                result.redundant_ignores.push(RedundantIgnore { name: ignored_dep.clone() });
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
                let dep = dep.get_ref();
                let pkg = dependency.get_ref().package().map_or_else(|| dep.clone(), str::to_owned);
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
}
