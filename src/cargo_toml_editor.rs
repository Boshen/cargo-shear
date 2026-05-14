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
        // Lift the entry out of `[dependencies]` so we can re-insert it elsewhere.
        let Some(value) = manifest
            .get_mut("dependencies")
            .and_then(|item| item.as_table_mut())
            .and_then(|deps| deps.remove(dep.name.get_ref()))
        else {
            return false;
        };

        // Create `[dev-dependencies]` on the fly if the manifest doesn't have one yet.
        if !manifest.contains_key("dev-dependencies") {
            manifest["dev-dependencies"] = Item::Table(Table::new());
        }

        // Re-insert into `[dev-dependencies]`. The earlier `contains_key` check
        // means this should always succeed, but stay defensive in case of a
        // non-table value at that key.
        if let Some(dev_deps) =
            manifest.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
        {
            dev_deps.insert(dep.name.get_ref(), value);
            return true;
        }

        false
    }

    fn move_target_to_dev(manifest: &mut DocumentMut, dep: &MisplacedDependency) -> bool {
        let DepLocation::Target { cfg, .. } = &dep.location else {
            return false;
        };

        let Some(target) = manifest
            .get_mut("target")
            .and_then(|item| item.as_table_mut())
            .and_then(|targets| targets.get_mut(cfg))
            .and_then(|item| item.as_table_mut())
        else {
            return false;
        };

        // Lift the entry out of `[target.'cfg(...)'.dependencies]`.
        let Some(value) = target
            .get_mut("dependencies")
            .and_then(|item| item.as_table_mut())
            .and_then(|deps| deps.remove(dep.name.get_ref()))
        else {
            return false;
        };

        // Create the dev-dependencies table for this `cfg` if it isn't there yet.
        if !target.contains_key("dev-dependencies") {
            target["dev-dependencies"] = Item::Table(Table::new());
        }

        // Re-insert into `[target.'cfg(...)'.dev-dependencies]`.
        if let Some(dev_deps) =
            target.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
        {
            dev_deps.insert(dep.name.get_ref(), value);
            return true;
        }

        false
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
        if !manifest.contains_key("lib") {
            manifest["lib"] = Item::Table(Table::new());
        }
        if let Some(table) = manifest.get_mut("lib").and_then(|item| item.as_table_mut()) {
            table[flag] = value(false);
        }
    }

    fn cleanup_empty_tables(manifest: &mut DocumentMut) {
        // Top-level `[dependencies]` / `[dev-dependencies]` / `[build-dependencies]`.
        for key in DEP_TABLE_KEYS {
            if manifest.get(key).and_then(|i| i.as_table()).is_some_and(Table::is_empty) {
                manifest.remove(key);
            }
        }

        // Target-specific tables, walked bottom-up: snapshot the keys first so
        // we can mutate `targets` without invalidating an in-flight iterator.
        if let Some(targets) = manifest.get_mut("target").and_then(|i| i.as_table_mut()) {
            let target_keys: Vec<String> = targets.iter().map(|(k, _)| k.to_owned()).collect();
            for cfg_key in &target_keys {
                if let Some(target) = targets.get_mut(cfg_key).and_then(|i| i.as_table_mut()) {
                    for dep_key in DEP_TABLE_KEYS {
                        if target
                            .get(dep_key)
                            .and_then(|i| i.as_table())
                            .is_some_and(Table::is_empty)
                        {
                            target.remove(dep_key);
                        }
                    }
                }
                if targets.get(cfg_key).and_then(|i| i.as_table()).is_some_and(Table::is_empty) {
                    targets.remove(cfg_key);
                }
            }
        }
        if manifest.get("target").and_then(|i| i.as_table()).is_some_and(Table::is_empty) {
            manifest.remove("target");
        }

        // `[workspace.dependencies]` — only drop the `dependencies` sub-key, not
        // the whole `[workspace]` table (it carries members, resolver, etc.).
        if let Some(workspace) = manifest.get_mut("workspace").and_then(|i| i.as_table_mut())
            && workspace.get("dependencies").and_then(|i| i.as_table()).is_some_and(Table::is_empty)
        {
            workspace.remove("dependencies");
        }

        // `[lib]` — drop the table when it has no remaining keys.
        if manifest.get("lib").and_then(|i| i.as_table()).is_some_and(Table::is_empty) {
            manifest.remove("lib");
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
