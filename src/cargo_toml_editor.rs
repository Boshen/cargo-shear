//! Format-preserving rewrites for `Cargo.toml`. Built on `toml_edit`, so
//! comments, ordering, and styling survive the edit.
//!
//! Covers:
//! - Package-level dependencies (`[dependencies]`, `[dev-dependencies]`, ...).
//! - Workspace dependencies (`[workspace.dependencies]`).
//! - Target-specific dependencies (`[target.'cfg(...)'.{table}]`).
//! - `[features]` entries that name removed dependencies.
//! - Lib-target flags `test` / `doctest`.

use rustc_hash::FxHashSet;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::{
    manifest::DepLocation,
    package_processor::{MisplacedDependency, UnusedDependency, UnusedWorkspaceDependency},
};

const DEP_TABLE_KEYS: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Stateless namespace for the `Cargo.toml` rewrite operations.
pub struct CargoTomlEditor;

impl CargoTomlEditor {
    /// Remove each entry in `unused_deps` from its declared location, scrub any
    /// dangling references from `[features]`, and prune now-empty parent tables.
    ///
    /// Returns the number of dependencies actually removed (entries that no
    /// longer exist in the manifest are silently skipped).
    pub fn remove_dependencies(
        manifest: &mut DocumentMut,
        unused_deps: &[UnusedDependency],
    ) -> usize {
        let mut removed = FxHashSet::default();

        for dep in unused_deps {
            let success = match &dep.location {
                DepLocation::Root(table) => manifest
                    .get_mut(&table.to_string())
                    .and_then(|item| item.as_table_mut())
                    .and_then(|deps| deps.remove(dep.name.get_ref()))
                    .is_some(),

                DepLocation::Target { cfg, table } => manifest
                    .get_mut("target")
                    .and_then(|item| item.as_table_mut())
                    .and_then(|targets| targets.get_mut(cfg))
                    .and_then(|item| item.as_table_mut())
                    .and_then(|target| target.get_mut(&table.to_string()))
                    .and_then(|item| item.as_table_mut())
                    .and_then(|deps| deps.remove(dep.name.get_ref()))
                    .is_some(),
            };

            if success {
                removed.insert(dep.name.get_ref().as_str());
            }
        }

        let count = removed.len();
        Self::fix_features(manifest, &removed);
        Self::cleanup_empty_tables(manifest);
        count
    }

    /// Remove each entry in `unused_deps` from `[workspace.dependencies]`,
    /// scrub any dangling references from `[features]`, and prune the
    /// containing tables if they end up empty.
    ///
    /// Returns the number of dependencies actually removed.
    pub fn remove_workspace_deps(
        manifest: &mut DocumentMut,
        unused_deps: &[UnusedWorkspaceDependency],
    ) -> usize {
        let mut removed = FxHashSet::default();

        for dep in unused_deps {
            let success = manifest
                .get_mut("workspace")
                .and_then(|item| item.as_table_mut())
                .and_then(|workspace| workspace.get_mut("dependencies"))
                .and_then(|item| item.as_table_mut())
                .and_then(|deps| deps.remove(dep.name.get_ref()))
                .is_some();

            if success {
                removed.insert(dep.name.get_ref().as_str());
            }
        }

        let count = removed.len();
        Self::fix_features(manifest, &removed);
        Self::cleanup_empty_tables(manifest);
        count
    }

    /// Move each entry from its current (non-dev) table to the matching
    /// `dev-dependencies` table, preserving its `cfg` target if any.
    ///
    /// Returns the number of dependencies actually moved.
    pub fn move_to_dev_dependencies(
        manifest: &mut DocumentMut,
        misplaced_deps: &[MisplacedDependency],
    ) -> usize {
        let mut count = 0;

        for dep in misplaced_deps {
            let success = match &dep.location {
                DepLocation::Root(_) => Self::move_root_to_dev(manifest, dep),
                DepLocation::Target { .. } => Self::move_target_to_dev(manifest, dep),
            };

            if success {
                count += 1;
            }
        }

        Self::cleanup_empty_tables(manifest);
        count
    }

    fn move_root_to_dev(manifest: &mut DocumentMut, dep: &MisplacedDependency) -> bool {
        Self::move_to_dev(manifest.as_table_mut(), dep.name.get_ref())
    }

    fn move_target_to_dev(manifest: &mut DocumentMut, dep: &MisplacedDependency) -> bool {
        let DepLocation::Target { cfg, .. } = &dep.location else {
            return false;
        };

        manifest
            .get_mut("target")
            .and_then(|item| item.as_table_mut())
            .and_then(|targets| targets.get_mut(cfg))
            .and_then(|item| item.as_table_mut())
            .is_some_and(|target| Self::move_to_dev(target, dep.name.get_ref()))
    }

    /// Move `name` from `parent.dependencies` to `parent.dev-dependencies`,
    /// creating the latter if needed. `parent` is either the document root or
    /// a `[target.'cfg(...)']` table.
    fn move_to_dev(parent: &mut Table, name: &str) -> bool {
        let Some(value) = parent
            .get_mut("dependencies")
            .and_then(|item| item.as_table_mut())
            .and_then(|deps| deps.remove(name))
        else {
            return false;
        };

        // Stay defensive in case `dev-dependencies` exists but isn't a table.
        let Some(dev_deps) = parent
            .entry("dev-dependencies")
            .or_insert_with(|| Item::Table(Table::new()))
            .as_table_mut()
        else {
            return false;
        };

        dev_deps.insert(name, value);
        true
    }

    /// Remove a single flag (e.g. `test` or `doctest`) from the `[lib]` table.
    /// Returns `true` if the key was present and removed.
    pub fn remove_lib_flag(manifest: &mut DocumentMut, flag: &str) -> bool {
        let removed = manifest
            .get_mut("lib")
            .and_then(|item| item.as_table_mut())
            .and_then(|table| table.remove(flag))
            .is_some();
        if removed {
            Self::cleanup_empty_tables(manifest);
        }
        removed
    }

    /// Force a flag to `false` under `[lib]`, creating the table if it's absent.
    /// Used to suppress Cargo's defaults of `test = true` / `doctest = true`.
    pub fn set_lib_flag_false(manifest: &mut DocumentMut, flag: &str) {
        if let Some(table) =
            manifest.entry("lib").or_insert_with(|| Item::Table(Table::new())).as_table_mut()
        {
            table[flag] = value(false);
        }
    }

    fn cleanup_empty_tables(manifest: &mut DocumentMut) {
        let root = manifest.as_table_mut();

        // Top-level `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`.
        for key in DEP_TABLE_KEYS {
            Self::remove_if_empty(root, key);
        }

        // Target-specific tables, walked bottom-up: snapshot the keys first so
        // we can mutate `targets` without invalidating an in-flight iterator.
        if let Some(targets) = root.get_mut("target").and_then(|i| i.as_table_mut()) {
            let target_keys: Vec<String> = targets.iter().map(|(k, _)| k.to_owned()).collect();
            for cfg_key in &target_keys {
                if let Some(target) = targets.get_mut(cfg_key).and_then(|i| i.as_table_mut()) {
                    for dep_key in DEP_TABLE_KEYS {
                        Self::remove_if_empty(target, dep_key);
                    }
                }
                Self::remove_if_empty(targets, cfg_key);
            }
        }
        Self::remove_if_empty(root, "target");

        // `[workspace.dependencies]` — only drop the `dependencies` sub-key, not
        // the whole `[workspace]` table (it carries members, resolver, etc.).
        if let Some(workspace) = root.get_mut("workspace").and_then(|i| i.as_table_mut()) {
            Self::remove_if_empty(workspace, "dependencies");
        }

        // `[lib]` — drop the table when it has no remaining keys.
        Self::remove_if_empty(root, "lib");
    }

    /// Remove `table[key]` if it is an empty table.
    fn remove_if_empty(table: &mut Table, key: &str) {
        if table.get(key).and_then(|i| i.as_table()).is_some_and(Table::is_empty) {
            table.remove(key);
        }
    }

    fn fix_features(manifest: &mut DocumentMut, unused_deps: &FxHashSet<&str>) {
        let Some(features) = manifest.get_mut("features").and_then(|item| item.as_table_mut())
        else {
            return;
        };

        for (_, deps) in features.iter_mut() {
            let Some(list) = deps.as_array_mut() else {
                continue;
            };

            list.retain(|value| {
                let Some(value) = value.as_str() else {
                    return true;
                };

                let dep = value.strip_prefix("dep:").unwrap_or(value);
                !unused_deps.contains(dep)
            });
        }
    }
}
