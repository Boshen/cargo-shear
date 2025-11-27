//! Cargo.toml editing module for cargo-shear.
//!
//! This module provides functionality to safely remove and move dependencies in
//! Cargo.toml files while preserving formatting and other content.
//!
//! It handles:
//! - Package-level dependencies (`[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`)
//! - Workspace dependencies (`[workspace.dependencies]`)
//! - Target specific dependencies (`[target.'cfg(...)'.dependencies]`)
//! - Feature flags that reference dependencies

use rustc_hash::{FxHashMap, FxHashSet};
use toml_edit::{DocumentMut, Item, Table};

use crate::{
    diagnostic::{MisplacedDependency, UnusedDependency, UnusedWorkspaceDependency},
    manifest::{DepLocation, DepTable},
};

/// Provides methods to edit Cargo.toml files.
pub struct CargoTomlEditor;

impl CargoTomlEditor {
    /// Remove unused dependencies from a manifest.
    ///
    /// This method will:
    /// 1. Remove dependencies from `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]`
    /// 2. Remove dependencies from `[target.'cfg'.dependencies]` tables
    /// 3. Remove dependencies from `[workspace.dependencies]` if in a workspace root
    /// 4. Update feature flags to remove references to removed dependencies
    pub fn remove_dependencies(manifest: &mut DocumentMut, unused: &[UnusedDependency]) {
        if unused.is_empty() {
            return;
        }

        let deps: FxHashSet<&str> = unused.iter().map(|d| d.name.as_str()).collect();
        Self::remove_package_dependencies(manifest, &deps);
        Self::remove_target_dependencies(manifest, &deps);
        Self::fix_features(manifest, &deps);
    }

    /// Remove unused workspace dependencies from a manifest.
    pub fn remove_workspace_deps(manifest: &mut DocumentMut, unused: &[UnusedWorkspaceDependency]) {
        if unused.is_empty() {
            return;
        }

        let deps: FxHashSet<&str> = unused.iter().map(|d| d.name.as_str()).collect();
        Self::remove_workspace_dependencies(manifest, &deps);
        Self::fix_features(manifest, &deps);
    }

    /// Move misplaced dependencies in a manifest.
    ///
    /// This method will:
    /// 1. Move misplaced dependencies from `[dependencies]` to `[dev-dependencies]`
    /// 2. Move misplaced dependencies from `[target.'cfg'.dependencies]` to `[target.'cfg'.dev-dependencies]`
    pub fn move_to_dev_dependencies(manifest: &mut DocumentMut, misplaced: &[MisplacedDependency]) {
        if misplaced.is_empty() {
            return;
        }

        let mut package_deps = vec![];
        let mut target_deps: FxHashMap<&str, Vec<&str>> = FxHashMap::default();

        for diag in misplaced {
            match &diag.location {
                DepLocation::Root(DepTable::Normal) => {
                    package_deps.push(diag.name.as_str());
                }
                DepLocation::Target { cfg, table: DepTable::Normal } => {
                    target_deps.entry(cfg.as_str()).or_default().push(diag.name.as_str());
                }
                DepLocation::Root(_) | DepLocation::Target { .. } => {}
            }
        }

        Self::move_package_to_dev(manifest, &package_deps);
        Self::move_target_to_dev(manifest, &target_deps);
    }

    fn move_package_to_dev(manifest: &mut DocumentMut, deps: &[&str]) {
        if deps.is_empty() {
            return;
        }

        let Some(dependencies) =
            manifest.get_mut("dependencies").and_then(|item| item.as_table_mut())
        else {
            return;
        };

        // Remove from `[dependencies]`
        let mut moved: Vec<(&str, Item)> = vec![];
        for name in deps {
            if let Some(value) = dependencies.remove(name) {
                moved.push((name, value));
            }
        }

        if moved.is_empty() {
            return;
        }

        // Ensure `[dev-dependencies]` exists
        if !manifest.contains_key("dev-dependencies") {
            manifest["dev-dependencies"] = Item::Table(Table::new());
        }

        // Insert into `[dev-dependencies]`
        if let Some(dev_dependencies) =
            manifest.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
        {
            for (dep, value) in moved {
                dev_dependencies.insert(dep, value);
            }
        }
    }

    fn move_target_to_dev(manifest: &mut DocumentMut, target_deps: &FxHashMap<&str, Vec<&str>>) {
        let Some(target) = manifest.get_mut("target").and_then(|item| item.as_table_mut()) else {
            return;
        };

        for (cfg, deps) in target_deps {
            let Some(cfg) = target.get_mut(cfg).and_then(|item| item.as_table_mut()) else {
                continue;
            };

            let Some(dependencies) =
                cfg.get_mut("dependencies").and_then(|item| item.as_table_mut())
            else {
                continue;
            };

            // Remove from `[target.'cfg'.dependencies]`
            let mut moved: Vec<(&str, Item)> = vec![];
            for name in deps {
                if let Some(value) = dependencies.remove(name) {
                    moved.push((name, value));
                }
            }

            if moved.is_empty() {
                continue;
            }

            // Ensure `[target.'cfg'.dev-dependencies]` exists
            if !cfg.contains_key("dev-dependencies") {
                cfg["dev-dependencies"] = Item::Table(Table::new());
            }

            // Insert into `[target.'cfg'.dev-dependencies]`
            if let Some(dev_dependencies) =
                cfg.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
            {
                for (dep, value) in moved {
                    dev_dependencies.insert(dep, value);
                }
            }
        }
    }

    fn remove_workspace_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<&str>) {
        if let Some(dependencies) = manifest
            .get_mut("workspace")
            .and_then(|item| item.as_table_mut())
            .and_then(|table| table.get_mut("dependencies"))
            .and_then(|item| item.as_table_mut())
        {
            dependencies.retain(|k, _| !unused_deps.contains(k));
        }
    }

    fn remove_package_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<&str>) {
        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) =
                manifest.get_mut(table_name).and_then(|item| item.as_table_mut())
            {
                dependencies.retain(|k, _| !unused_deps.contains(k));
            }
        }
    }

    fn remove_target_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<&str>) {
        let Some(target) = manifest.get_mut("target").and_then(|item| item.as_table_mut()) else {
            return;
        };

        for (_, item) in target.iter_mut() {
            let Some(table) = item.as_table_mut() else {
                continue;
            };

            for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(dependencies) = table.get_mut(name).and_then(|item| item.as_table_mut())
                {
                    dependencies.retain(|k, _| !unused_deps.contains(k));
                }
            }
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
