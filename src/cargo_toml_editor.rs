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
use std::fs;
use std::path::Path;
use std::str::FromStr;

use toml_edit::DocumentMut;

use anyhow::Result;

/// Provides methods to edit Cargo.toml files and remove unused dependencies.
pub struct CargoTomlEditor;

impl CargoTomlEditor {
    /// Remove unused dependencies from a Cargo.toml file.
    ///
    /// This method will:
    /// 1. Remove dependencies from `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]`
    /// 2. Remove dependencies from `[workspace.dependencies]` if in a workspace root
    /// 3. Update feature flags to remove references to removed dependencies
    ///
    /// # Arguments
    ///
    /// * `cargo_toml_path` - Path to the Cargo.toml file to edit
    /// * `unused_deps` - Set of dependency names to remove
    ///
    /// # Returns
    ///
    /// The number of dependencies that were removed
    pub fn remove_dependencies(
        cargo_toml_path: &Path,
        unused_deps: &FxHashSet<String>,
    ) -> Result<usize> {
        if unused_deps.is_empty() {
            return Ok(0);
        }

        let manifest = fs::read_to_string(cargo_toml_path)?;
        let mut manifest = DocumentMut::from_str(&manifest)?;

        Self::remove_workspace_dependencies(&mut manifest, unused_deps);
        Self::remove_package_dependencies(&mut manifest, unused_deps);
        Self::fix_features(&mut manifest, unused_deps);

        let serialized = manifest.to_string();
        fs::write(cargo_toml_path, serialized)?;

        Ok(unused_deps.len())
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
