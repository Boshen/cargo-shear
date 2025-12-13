//! Analyze packages to identify issues.
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

use std::path::{Path, PathBuf};

use anyhow::Result;
use rustc_hash::FxHashSet;
use toml::Spanned;

use crate::{
    context::{PackageContext, WorkspaceContext},
    manifest::{DepLocation, FeatureRef},
    package_analyzer::PackageAnalyzer,
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

/// An unlinked file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnlinkedFile {
    /// The relative path to the unlinked file.
    pub path: PathBuf,
}

/// An unknown ignore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownIgnore {
    /// The dependency key.
    pub name: Spanned<String>,
}

/// A redundant ignore.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RedundantIgnore {
    /// The dependency key.
    pub name: Spanned<String>,
}

/// A redundant ignored path pattern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RedundantIgnorePath {
    /// The redundant glob pattern.
    pub pattern: String,
}

/// An empty file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmptyFile {
    /// The relative path to the empty file.
    pub path: PathBuf,
}

/// Processes packages to identify issues.
pub struct PackageProcessor {
    /// Whether to use `cargo expand` to expand macros
    expand_macros: bool,
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

    /// Unlinked files.
    pub unlinked_files: Vec<UnlinkedFile>,

    /// Empty files.
    pub empty_files: Vec<EmptyFile>,

    /// Unknown ignores.
    pub unknown_ignores: Vec<UnknownIgnore>,

    /// Redundant ignores.
    pub redundant_ignores: Vec<RedundantIgnore>,

    /// Redundant ignored path patterns.
    pub redundant_ignore_paths: Vec<RedundantIgnorePath>,
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
        Self { expand_macros }
    }

    /// Process a package to find package level issues.
    #[expect(
        clippy::too_many_lines,
        reason = "Complex function handling multiple diagnostic types"
    )]
    pub fn process_package(&self, ctx: &PackageContext<'_>) -> Result<PackageAnalysis> {
        let analyzer = PackageAnalyzer::new(ctx, self.expand_macros);
        let used_imports = analyzer.analyze()?;

        let code_imports = used_imports.code_imports();
        let feature_imports = used_imports.feature_imports();

        let mut result = PackageAnalysis::default();

        // Collect used packages
        for (import, pkg) in &ctx.import_to_pkg {
            if code_imports.contains(import.as_str()) || feature_imports.contains(import.as_str()) {
                result.used_packages.insert(pkg.clone());
            }
        }

        // An ignore is only redundant if removing it wouldn't trigger any other diagnostics.
        let mut suppressed_ignores: FxHashSet<String> = FxHashSet::default();

        // Analyze dependencies
        for (dep, dependency, location) in ctx.manifest.all_dependencies() {
            let pkg = dependency.get_ref().package().unwrap_or_else(|| dep.get_ref().as_str());
            let import = ctx
                .pkg_to_import
                .get(pkg)
                .cloned()
                .unwrap_or_else(|| dep.get_ref().replace('-', "_"));

            let is_ignored = ctx.ignored_imports.contains(&import);

            if !code_imports.contains(&*import) {
                if is_ignored {
                    suppressed_ignores.insert(import);
                    continue;
                }

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
                if is_ignored {
                    suppressed_ignores.insert(import);
                    continue;
                }

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

        // Analyze ignores
        let package_ignored_deps = &ctx.manifest.package.metadata.cargo_shear.ignored;
        for ignored_dep in package_ignored_deps {
            let ignored_import = ignored_dep.get_ref().replace('-', "_");

            if !ctx.import_to_pkg.contains_key(&ignored_import) {
                result.unknown_ignores.push(UnknownIgnore { name: ignored_dep.clone() });
                continue;
            }

            if !suppressed_ignores.contains(&ignored_import) {
                result.redundant_ignores.push(RedundantIgnore { name: ignored_dep.clone() });
            }
        }

        // Analyze unlinked files
        let unlinked_files: FxHashSet<PathBuf> = used_imports
            .unlinked_files
            .iter()
            .filter_map(|path| path.strip_prefix(&ctx.directory).ok().map(Path::to_path_buf))
            .collect();

        let pkg_ignored_paths = &ctx.manifest.package.metadata.cargo_shear.ignored_paths;
        let ws_ignored_paths = &ctx.workspace.manifest.workspace.metadata.cargo_shear.ignored_paths;

        // Ensure ignores are relative to package directory
        let root = ctx.directory.strip_prefix(&ctx.workspace.root).unwrap_or(&ctx.directory);

        result.redundant_ignore_paths = pkg_ignored_paths
            .iter()
            .filter(|matcher| !unlinked_files.iter().any(|path| matcher.is_match(path)))
            .map(|matcher| RedundantIgnorePath { pattern: matcher.glob().glob().to_owned() })
            .collect();

        result.unlinked_files = unlinked_files
            .into_iter()
            .filter(|path| {
                !pkg_ignored_paths.iter().any(|globs| globs.is_match(path))
                    && !ws_ignored_paths.iter().any(|globs| globs.is_match(root.join(path)))
            })
            .map(|path| UnlinkedFile { path })
            .collect();

        // Process empty files
        result.empty_files = used_imports
            .empty_files
            .iter()
            .filter_map(|path| path.strip_prefix(&ctx.directory).ok().map(Path::to_path_buf))
            .filter(|path| {
                !pkg_ignored_paths.iter().any(|globs| globs.is_match(path))
                    && !ws_ignored_paths.iter().any(|globs| globs.is_match(root.join(path)))
            })
            .map(|path| EmptyFile { path })
            .collect();

        Ok(result)
    }

    /// Process workspace to find workspace level issues.
    pub fn process_workspace(
        ctx: &WorkspaceContext,
        workspace_used_pkgs: &FxHashSet<String>,
    ) -> WorkspaceAnalysis {
        let mut result = WorkspaceAnalysis::default();

        if ctx.packages <= 1 {
            return result;
        }

        if ctx.manifest.workspace.dependencies.is_empty() {
            return result;
        }

        for (dep, dependency) in &ctx.manifest.workspace.dependencies {
            if ctx.ignored_deps.contains(dep.get_ref()) {
                continue;
            }

            let pkg = dependency.get_ref().package().unwrap_or(dep.get_ref());
            if !workspace_used_pkgs.contains(pkg) {
                result.unused_dependencies.push(UnusedWorkspaceDependency { name: dep.clone() });
            }
        }

        let ignored_deps = &ctx.manifest.workspace.metadata.cargo_shear.ignored;
        for ignored_dep in ignored_deps {
            if !ctx.dep_to_pkg.contains_key(ignored_dep.get_ref()) {
                result.unknown_ignores.push(UnknownIgnore { name: ignored_dep.clone() });
                continue;
            }

            if ctx
                .dep_to_pkg
                .get(ignored_dep.get_ref())
                .is_some_and(|pkg| workspace_used_pkgs.contains(pkg))
            {
                result.redundant_ignores.push(RedundantIgnore { name: ignored_dep.clone() });
            }
        }

        result
    }
}
