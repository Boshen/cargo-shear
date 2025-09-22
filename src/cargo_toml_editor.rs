use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::str::FromStr;

use toml_edit::DocumentMut;

use crate::error::Result;

pub struct CargoTomlEditor;

impl CargoTomlEditor {
    pub fn remove_dependencies(
        cargo_toml_path: &Path,
        unused_deps: &HashSet<String>,
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

    fn remove_workspace_dependencies(manifest: &mut DocumentMut, unused_deps: &HashSet<String>) {
        if let Some(dependencies) = manifest
            .get_mut("workspace")
            .and_then(|item| item.as_table_mut())
            .and_then(|table| table.get_mut("dependencies"))
            .and_then(|item| item.as_table_mut())
        {
            dependencies.retain(|k, _| !unused_deps.contains(k));
        }
    }

    fn remove_package_dependencies(manifest: &mut DocumentMut, unused_deps: &HashSet<String>) {
        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) = manifest
                .get_mut(table_name)
                .and_then(|item| item.as_table_mut())
            {
                dependencies.retain(|k, _| !unused_deps.contains(k));
            }
        }
    }

    fn fix_features(manifest: &mut DocumentMut, unused_deps: &HashSet<String>) {
        if let Some(features) = manifest
            .get_mut("features")
            .and_then(|item| item.as_table_mut())
        {
            for (_feature_name, dependencies) in features.iter_mut() {
                if let Some(dependencies) = dependencies.as_array_mut() {
                    dependencies.retain(|dep| {
                        dep.as_str().is_none_or(|dep_str| {
                            dep_str.strip_prefix("dep:")
                                .map_or_else(|| !unused_deps.contains(dep_str), |dep_name| !unused_deps.contains(dep_name))
                        })
                    });
                }
            }
        }
    }
}