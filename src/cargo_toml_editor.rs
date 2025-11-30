//! Cargo.toml editing module for cargo-shear.
//!
//! This module provides functionality to safely remove unused dependencies
//! from Cargo.toml files while preserving formatting and other content.
//! It handles:
//!
//! - Package-level dependencies (`[dependencies]`, `[dev-dependencies]`, etc.)
//! - Workspace dependencies (`[workspace.dependencies]`)
//! - Target specific dependencies (`[target.'cfg(...)'.dependencies]`)
//! - Feature flags that reference removed dependencies

use rustc_hash::FxHashSet;
use toml_edit::{DocumentMut, Item, Table};

use crate::{
    manifest::DepLocation,
    package_processor::{MisplacedDependency, UnusedDependency, UnusedWorkspaceDependency},
};

/// Provides methods to edit Cargo.toml files and remove unused dependencies.
pub struct CargoTomlEditor;

impl CargoTomlEditor {
    /// Remove unused dependencies from a manifest.
    ///
    /// # Returns
    ///
    /// The number of dependencies removed.
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
        count
    }

    /// Remove unused workspace dependencies from a manifest.
    ///
    /// # Returns
    ///
    /// The number of dependencies removed.
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
        count
    }

    /// Move dependencies from `[dependencies]` to `[dev-dependencies]`.
    ///
    /// # Returns
    ///
    /// The number of dependencies moved.
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

        count
    }

    fn move_root_to_dev(manifest: &mut DocumentMut, dep: &MisplacedDependency) -> bool {
        // Remove from `[dependencies]`
        let Some(value) = manifest
            .get_mut("dependencies")
            .and_then(|item| item.as_table_mut())
            .and_then(|deps| deps.remove(dep.name.get_ref()))
        else {
            return false;
        };

        // Ensure `[dev-dependencies]` exists
        if !manifest.contains_key("dev-dependencies") {
            manifest["dev-dependencies"] = Item::Table(Table::new());
        }

        // Insert into `[dev-dependencies]`
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

        // Remove from `[target.'cfg(...)'.dependencies]`
        let Some(value) = target
            .get_mut("dependencies")
            .and_then(|item| item.as_table_mut())
            .and_then(|deps| deps.remove(dep.name.get_ref()))
        else {
            return false;
        };

        // Ensure `[target.'cfg(...)'.dev-dependencies]` exists
        if !target.contains_key("dev-dependencies") {
            target["dev-dependencies"] = Item::Table(Table::new());
        }

        // Insert into `[target.'cfg(...)'.dev-dependencies]`
        if let Some(dev_deps) =
            target.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
        {
            dev_deps.insert(dep.name.get_ref(), value);
            return true;
        }

        false
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
