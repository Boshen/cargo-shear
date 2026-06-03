//! Analyze packages to identify issues.
//!
//! # Terminology
//!
//! * import: Imports from within Rust code:
//!
//! ```rust,ignore
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
    manifest::{DepLocation, FeatureRef, lib_kind_label},
    package_analyzer::PackageAnalyzer,
};

/// A dependency declared in a manifest but never imported by any target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedDependency {
    /// Dependency key as it appears in `[dependencies]` (or a target/dev/build variant).
    pub name: Spanned<String>,

    /// Which dependency table the entry lives in.
    pub location: DepLocation,
}

/// An `optional = true` dependency that no enabled feature pulls in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedOptionalDependency {
    /// Dependency key as it appears in `[dependencies]`.
    pub name: Spanned<String>,

    /// Feature entries that reference this dependency, for the diagnostic to render.
    pub features: Vec<FeatureRef>,
}

/// A dependency only referenced from `[features]`, never imported in code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedFeatureDependency {
    /// Dependency key as it appears in `[dependencies]`.
    pub name: Spanned<String>,

    /// Feature entries that reference this dependency.
    pub features: Vec<FeatureRef>,
}

/// A `[workspace.dependencies]` entry not used by any workspace member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnusedWorkspaceDependency {
    /// Dependency key as it appears in `[workspace.dependencies]`.
    pub name: Spanned<String>,
}

/// A dependency in `[dependencies]` whose only usages are dev/test/bench/example targets.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedDependency {
    /// Dependency key as it appears in the (non-dev) dependency table.
    pub name: Spanned<String>,

    /// Which dependency table the entry currently lives in.
    pub location: DepLocation,
}

/// A misplaced dependency that is also marked `optional`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MisplacedOptionalDependency {
    /// Dependency key as it appears in the (non-dev) dependency table.
    pub name: Spanned<String>,

    /// Which dependency table the entry currently lives in.
    pub location: DepLocation,

    /// Feature entries that reference this dependency.
    pub features: Vec<FeatureRef>,
}

/// A source file that exists on disk but isn't reachable from any module tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnlinkedFile {
    /// Path relative to the package directory.
    pub path: PathBuf,
}

/// An entry in `ignored = [...]` that doesn't match any declared dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnknownIgnore {
    /// The unrecognised dependency key.
    pub name: Spanned<String>,
}

/// An ignore entry whose dependency is actually used, so the ignore is suppressing nothing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RedundantIgnore {
    /// The dependency key listed in `ignored = [...]`.
    pub name: Spanned<String>,
}

/// A glob in `ignored-paths` that matches no unlinked or empty file.
#[derive(Debug, Clone)]
pub struct RedundantIgnorePath {
    /// The glob pattern as written in the manifest.
    pub pattern: Spanned<String>,
}

/// A linked source file that contains no items (only whitespace/comments).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmptyFile {
    /// Path relative to the package directory.
    pub path: PathBuf,
}

/// A target with `test = false` whose source still contains tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestDisabledWithTests {
    /// Cargo target name.
    pub target_name: String,
    /// Target kind label (e.g. `lib`, `rlib`, `proc-macro`).
    pub target_kind: String,
}

/// A target with `test = true` (the default) whose source contains no tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TestEnabledWithoutTests {
    /// Cargo target name.
    pub target_name: String,
    /// Target kind label (e.g. `lib`, `rlib`, `proc-macro`).
    pub target_kind: String,
}

/// A lib target with `doctest = false` whose source still contains doc tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DoctestDisabledWithDoctests {
    /// Cargo target name.
    pub target_name: String,
}

/// A lib target with `doctest = true` (the default) whose source contains no doc tests.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DoctestEnabledWithoutDoctests {
    /// Cargo target name.
    pub target_name: String,
}

/// Drives per-package and workspace-level analysis.
pub struct PackageProcessor {
    /// Run `cargo expand` first so macro-generated imports become visible (slower; nightly only).
    expand_macros: bool,

    /// Whether to emit test/doctest flag mismatch diagnostics.
    check_test_targets: bool,
}

/// Diagnostics collected for a single package.
#[derive(Default)]
pub struct PackageAnalysis {
    /// Package names whose imports were detected in this package's source.
    pub used_packages: FxHashSet<String>,

    /// Non-optional dependencies that no target imports.
    pub unused_dependencies: Vec<UnusedDependency>,

    /// `optional = true` dependencies not enabled by any imported feature.
    pub unused_optional_dependencies: Vec<UnusedOptionalDependency>,

    /// Dependencies only referenced from `[features]`, never imported in code.
    pub unused_feature_dependencies: Vec<UnusedFeatureDependency>,

    /// Dependencies that should move from `[dependencies]` to `[dev-dependencies]`.
    pub misplaced_dependencies: Vec<MisplacedDependency>,

    /// Misplaced dependencies that are also marked `optional`.
    pub misplaced_optional_dependencies: Vec<MisplacedOptionalDependency>,

    /// Source files not reachable from any entry point.
    pub unlinked_files: Vec<UnlinkedFile>,

    /// Source files reachable from an entry point but containing no items.
    pub empty_files: Vec<EmptyFile>,

    /// `ignored = [...]` entries that don't match any declared dependency.
    pub unknown_ignores: Vec<UnknownIgnore>,

    /// `ignored = [...]` entries whose dependency is actually in use.
    pub redundant_ignores: Vec<RedundantIgnore>,

    /// `ignored-paths` globs that matched neither an unlinked nor an empty file.
    pub redundant_ignore_paths: Vec<RedundantIgnorePath>,

    /// Workspace-level `ignored-paths` globs that matched a file in this package
    /// (used to suppress workspace-level "redundant ignore path" diagnostics).
    pub used_workspace_ignore_paths: FxHashSet<String>,

    /// Lib-like targets with `test = false` whose source still contains tests.
    pub test_disabled_with_tests: Vec<TestDisabledWithTests>,

    /// Lib-like targets that default to `test = true` but contain no tests.
    pub test_enabled_without_tests: Vec<TestEnabledWithoutTests>,

    /// Lib targets with `doctest = false` whose source still contains doc tests.
    pub doctest_disabled_with_doctests: Vec<DoctestDisabledWithDoctests>,

    /// Lib targets that default to `doctest = true` but contain no doc tests.
    pub doctest_enabled_without_doctests: Vec<DoctestEnabledWithoutDoctests>,
}

impl PackageAnalysis {
    pub const fn has_fixable_issues(&self) -> bool {
        !self.misplaced_dependencies.is_empty()
            || !self.unused_dependencies.is_empty()
            || !self.test_disabled_with_tests.is_empty()
            || !self.test_enabled_without_tests.is_empty()
            || !self.doctest_disabled_with_doctests.is_empty()
            || !self.doctest_enabled_without_doctests.is_empty()
    }
}

/// Diagnostics collected for the workspace root manifest itself.
#[derive(Default)]
pub struct WorkspaceAnalysis {
    /// `[workspace.dependencies]` entries no member depends on.
    pub unused_dependencies: Vec<UnusedWorkspaceDependency>,

    /// `ignored = [...]` entries that don't match any workspace dependency key.
    pub unknown_ignores: Vec<UnknownIgnore>,

    /// `ignored = [...]` entries whose dependency is actually used.
    pub redundant_ignores: Vec<RedundantIgnore>,

    /// `ignored-paths` globs that didn't match any file in any package.
    pub redundant_ignore_paths: Vec<RedundantIgnorePath>,
}

impl PackageProcessor {
    pub const fn new(expand_macros: bool, check_test_targets: bool) -> Self {
        Self { expand_macros, check_test_targets }
    }

    /// Run all per-package diagnostics.
    #[expect(
        clippy::too_many_lines,
        reason = "Complex function handling multiple diagnostic types"
    )]
    pub fn process_package(&self, ctx: &PackageContext<'_>) -> Result<PackageAnalysis> {
        let analyzer = PackageAnalyzer::new(ctx, self.expand_macros);
        let used_imports = analyzer.analyze()?;

        let code_imports = used_imports.code_imports();
        let feature_imports = used_imports.feature_imports(&code_imports);

        let mut result = PackageAnalysis::default();

        // For each declared dep, mark the underlying package "used" if any of its imports show up.
        for (import, pkg) in &ctx.import_to_pkg {
            if code_imports.contains(import.as_str()) || feature_imports.contains(import.as_str()) {
                result.used_packages.insert(pkg.clone());
            }
        }

        // An ignore is only redundant if removing it wouldn't trigger any other diagnostics.
        let mut suppressed_ignores: FxHashSet<String> = FxHashSet::default();

        // Walk every dependency entry in this manifest and bucket it into a diagnostic (or none).
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
                    // Track ignored deps as used so the workspace analysis doesn't
                    // remove them from [workspace.dependencies].
                    // Only for package-level ignores; workspace-level ignores are
                    // already skipped by process_workspace via `ignored_deps`.
                    if !ctx.workspace.ignored_deps.contains(dep.get_ref().as_str()) {
                        result.used_packages.insert(pkg.to_owned());
                    }
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

        // Classify each `ignored = [...]` entry as unknown, redundant, or load-bearing.
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

        // Rebase paths to be package-relative so they match patterns in `ignored-paths`.
        let unlinked_files: FxHashSet<PathBuf> = used_imports
            .unlinked_files
            .iter()
            .filter_map(|path| path.strip_prefix(&ctx.directory).ok().map(Path::to_path_buf))
            .collect();

        let empty_files: FxHashSet<PathBuf> = used_imports
            .empty_files
            .iter()
            .filter_map(|path| path.strip_prefix(&ctx.directory).ok().map(Path::to_path_buf))
            .collect();

        let pkg_ignored_paths = &ctx.manifest.package.metadata.cargo_shear.ignored_paths;
        let ws_ignored_paths = &ctx.workspace.manifest.workspace.metadata.cargo_shear.ignored_paths;

        // Workspace-level globs are written relative to the workspace root, so we'll
        // need to rejoin package-relative paths under this prefix when matching them.
        let root = ctx.directory.strip_prefix(&ctx.workspace.root).unwrap_or(&ctx.directory);

        // A package-level pattern is redundant only if it matches neither an unlinked
        // nor an empty file in this package.
        result.redundant_ignore_paths = pkg_ignored_paths
            .iter()
            .filter(|glob| {
                !unlinked_files.iter().any(|path| glob.matcher.is_match(path))
                    && !empty_files.iter().any(|path| glob.matcher.is_match(path))
            })
            .map(|glob| RedundantIgnorePath { pattern: glob.pattern.clone() })
            .collect();

        // Record workspace-level globs that actually matched something here, so the
        // workspace pass can distinguish them from globs that match nothing anywhere.
        // A pattern that's already covered by a package-level glob doesn't count.
        for glob in ws_ignored_paths {
            let matches_unlinked = unlinked_files.iter().any(|path| {
                let not_matched_by_pkg =
                    !pkg_ignored_paths.iter().any(|pkg| pkg.matcher.is_match(path));
                not_matched_by_pkg && glob.matcher.is_match(root.join(path))
            });

            let matches_empty = empty_files.iter().any(|path| {
                let not_matched_by_pkg =
                    !pkg_ignored_paths.iter().any(|pkg| pkg.matcher.is_match(path));
                not_matched_by_pkg && glob.matcher.is_match(root.join(path))
            });

            if matches_unlinked || matches_empty {
                result.used_workspace_ignore_paths.insert(glob.pattern.get_ref().clone());
            }
        }

        // Drop files matched by either a package- or workspace-level ignore.
        result.unlinked_files = unlinked_files
            .into_iter()
            .filter(|path| {
                !pkg_ignored_paths.iter().any(|glob| glob.matcher.is_match(path))
                    && !ws_ignored_paths.iter().any(|glob| glob.matcher.is_match(root.join(path)))
            })
            .map(|path| UnlinkedFile { path })
            .collect();

        result.empty_files = empty_files
            .into_iter()
            .filter(|path| {
                !pkg_ignored_paths.iter().any(|glob| glob.matcher.is_match(path))
                    && !ws_ignored_paths.iter().any(|glob| glob.matcher.is_match(root.join(path)))
            })
            .map(|path| EmptyFile { path })
            .collect();

        // Compare each lib-like target's `test`/`doctest` flag against actual source contents.
        if self.check_test_targets {
            let is_workspace = ctx.workspace.packages.len() > 1;
            for info in &used_imports.target_test_info {
                // Only lib-like targets populate `target_test_info`, so the
                // fallback is unreachable in practice.
                let kind_str = lib_kind_label(&info.target_kind).unwrap_or("lib");

                if !info.test_enabled && info.has_tests {
                    result.test_disabled_with_tests.push(TestDisabledWithTests {
                        target_name: info.target_name.clone(),
                        target_kind: kind_str.to_owned(),
                    });
                }

                if is_workspace && info.test_enabled && !info.has_tests {
                    result.test_enabled_without_tests.push(TestEnabledWithoutTests {
                        target_name: info.target_name.clone(),
                        target_kind: kind_str.to_owned(),
                    });
                }

                if !info.doctest_enabled && info.has_doctests {
                    result.doctest_disabled_with_doctests.push(DoctestDisabledWithDoctests {
                        target_name: info.target_name.clone(),
                    });
                }

                if is_workspace && info.doctest_enabled && !info.has_doctests {
                    result.doctest_enabled_without_doctests.push(DoctestEnabledWithoutDoctests {
                        target_name: info.target_name.clone(),
                    });
                }
            }
        }

        Ok(result)
    }

    /// Run diagnostics that only make sense at the workspace level (i.e. require
    /// the per-package "used" sets to already be merged together).
    pub fn process_workspace(
        ctx: &WorkspaceContext,
        workspace_used_pkgs: &FxHashSet<String>,
        used_workspace_ignore_paths: &FxHashSet<String>,
    ) -> WorkspaceAnalysis {
        let mut result = WorkspaceAnalysis::default();

        // Workspace-level glob is redundant if no package reported it as load-bearing.
        let ws_ignored_paths = &ctx.manifest.workspace.metadata.cargo_shear.ignored_paths;
        for glob in ws_ignored_paths {
            if !used_workspace_ignore_paths.contains(glob.pattern.get_ref()) {
                result
                    .redundant_ignore_paths
                    .push(RedundantIgnorePath { pattern: glob.pattern.clone() });
            }
        }

        if ctx.packages.len() <= 1 || ctx.manifest.workspace.dependencies.is_empty() {
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
