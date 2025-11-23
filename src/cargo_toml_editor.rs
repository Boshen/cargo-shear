//! Cargo.toml editing module for cargo-shear.
//!
//! This module provides functionality to safely remove unused dependencies
//! from Cargo.toml files while preserving formatting and other content.
//! It handles:
//!
//! - Package-level dependencies (`[dependencies]`, `[dev-dependencies]`, etc.)
//! - Workspace dependencies (`[workspace.dependencies]`)
//! - Feature flags that reference removed dependencies

use rustc_hash::FxHashSet;
use toml_edit::{DocumentMut, Item, Table};

/// Provides methods to edit Cargo.toml files and remove unused dependencies.
pub struct CargoTomlEditor;

impl CargoTomlEditor {
    /// Remove unused dependencies from a manifest.
    ///
    /// This method will:
    /// 1. Remove dependencies from `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]`
    /// 2. Remove dependencies from `[workspace.dependencies]` if in a workspace root
    /// 3. Update feature flags to remove references to removed dependencies
    ///
    /// # Arguments
    ///
    /// * `manifest` - The manifest document to edit
    /// * `unused_deps` - Set of unused dependency keys
    ///
    /// # Returns
    ///
    /// The number of dependencies that were removed
    pub fn remove_dependencies(
        manifest: &mut DocumentMut,
        unused_deps: &FxHashSet<String>,
    ) -> usize {
        if unused_deps.is_empty() {
            return 0;
        }

        Self::remove_workspace_dependencies(manifest, unused_deps);
        Self::remove_package_dependencies(manifest, unused_deps);
        Self::remove_target_dependencies(manifest, unused_deps);
        Self::fix_features(manifest, unused_deps);

        unused_deps.len()
    }

    /// Move dependencies from `[dependencies]` to `[dev-dependencies]`.
    ///
    /// This method will:
    /// 1. Remove dependencies from `[dependencies]`
    /// 2. Insert them into `[dev-dependencies]`
    ///
    /// # Arguments
    ///
    /// * `manifest` - The manifest document to edit
    /// * `misplaced_deps` - Set of misplaced dependency keys
    ///
    /// # Returns
    ///
    /// The number of dependencies that were moved
    pub fn move_to_dev_dependencies(
        manifest: &mut DocumentMut,
        misplaced_deps: &FxHashSet<String>,
    ) -> usize {
        if misplaced_deps.is_empty() {
            return 0;
        }

        // Remove from `[dependencies]`
        let mut moved = Vec::new();
        if let Some(dependencies) =
            manifest.get_mut("dependencies").and_then(|item| item.as_table_mut())
        {
            for dep in misplaced_deps {
                if let Some(value) = dependencies.remove(dep.as_str()) {
                    moved.push((dep.clone(), value));
                }
            }
        }

        if moved.is_empty() {
            return 0;
        }

        let count = moved.len();

        // Ensure `[dev-dependencies]` exists
        if !manifest.contains_key("dev-dependencies") {
            manifest["dev-dependencies"] = Item::Table(Table::new());
        }

        // Insert into `[dev-dependencies]`
        if let Some(dev_deps) =
            manifest.get_mut("dev-dependencies").and_then(|item| item.as_table_mut())
        {
            for (dep, value) in moved {
                dev_deps.insert(&dep, value);
            }
        }

        count
    }

    fn remove_workspace_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<String>) {
        if let Some(dependencies) = manifest
            .get_mut("workspace")
            .and_then(|item| item.as_table_mut())
            .and_then(|table| table.get_mut("dependencies"))
            .and_then(|item| item.as_table_mut())
        {
            dependencies.retain(|k, _| !unused_deps.contains(k));
        }
    }

    fn remove_package_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<String>) {
        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) =
                manifest.get_mut(table_name).and_then(|item| item.as_table_mut())
            {
                dependencies.retain(|k, _| !unused_deps.contains(k));
            }
        }
    }

    fn remove_target_dependencies(manifest: &mut DocumentMut, unused_deps: &FxHashSet<String>) {
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

    fn fix_features(manifest: &mut DocumentMut, unused_deps: &FxHashSet<String>) {
        if let Some(features) = manifest.get_mut("features").and_then(|item| item.as_table_mut()) {
            for (_feature_name, dependencies) in features.iter_mut() {
                if let Some(dependencies) = dependencies.as_array_mut() {
                    dependencies.retain(|dep| {
                        dep.as_str().is_none_or(|dep_str| {
                            dep_str.strip_prefix("dep:").map_or_else(
                                || !unused_deps.contains(dep_str),
                                |dep_name| !unused_deps.contains(dep_name),
                            )
                        })
                    });
                }
            }
        }
    }
}
