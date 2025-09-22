use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, Package};

use crate::dependency_analyzer::{Dependencies, DependencyAnalyzer};
use crate::error::{Error, Result};

pub struct PackageProcessor {
    analyzer: DependencyAnalyzer,
}

pub struct ProcessResult {
    pub unused_dependencies: HashSet<String>,
    pub remaining_dependencies: Dependencies,
}

impl PackageProcessor {
    pub fn new(expand_macros: bool) -> Self {
        Self {
            analyzer: DependencyAnalyzer::new(expand_macros),
        }
    }

    pub fn process_package(
        &self,
        metadata: &Metadata,
        package: &Package,
    ) -> Result<ProcessResult> {
        let _workspace_root = metadata.workspace_root.as_std_path();

        let mut ignored_names = DependencyAnalyzer::get_ignored_package_names(&package.metadata);
        ignored_names.extend(DependencyAnalyzer::get_ignored_package_names(
            &metadata.workspace_metadata,
        ));

        let this_package = metadata
            .resolve
            .as_ref()
            .ok_or_else(|| {
                Error::metadata(
                    "`cargo_metadata::MetadataCommand::no_deps` should not be called.".to_string(),
                )
            })?
            .nodes
            .iter()
            .find(|node| node.id == package.id)
            .ok_or_else(|| Error::package_not_found(package.name.to_string()))?;

        let package_dependency_names_map = self.build_dependency_map(
            &this_package.deps,
            &ignored_names,
        )?;

        let module_names_from_package_deps: HashSet<String> = package_dependency_names_map
            .keys()
            .cloned()
            .collect();

        let package_dependency_names: HashSet<String> = package_dependency_names_map
            .values()
            .cloned()
            .collect();

        let module_names_from_rust_files = self.analyzer.analyze_package(package)?;

        let unused_module_names: Vec<&String> = module_names_from_package_deps
            .difference(&module_names_from_rust_files)
            .collect();

        if unused_module_names.is_empty() {
            return Ok(ProcessResult {
                unused_dependencies: HashSet::new(),
                remaining_dependencies: package_dependency_names,
            });
        }

        let unused_dependency_names: HashSet<String> = unused_module_names
            .into_iter()
            .map(|name| package_dependency_names_map[name].clone())
            .collect();

        let remaining_dependencies = package_dependency_names
            .difference(&unused_dependency_names)
            .cloned()
            .collect();

        Ok(ProcessResult {
            unused_dependencies: unused_dependency_names,
            remaining_dependencies,
        })
    }

    pub fn process_workspace(
        &self,
        metadata: &Metadata,
        all_package_deps: &Dependencies,
    ) -> Result<HashSet<String>> {
        if metadata.workspace_packages().len() <= 1 {
            return Ok(HashSet::new());
        }

        let metadata_path = metadata.workspace_root.as_std_path();
        let cargo_toml_path = metadata_path.join("Cargo.toml");
        let manifest = cargo_toml::Manifest::from_path(&cargo_toml_path)?;

        let Some(workspace) = &manifest.workspace else {
            return Ok(HashSet::new());
        };

        let ignored_names = DependencyAnalyzer::get_ignored_package_names(
            &metadata.workspace_metadata,
        );

        let workspace_deps: HashSet<String> = workspace
            .dependencies
            .iter()
            .map(|(key, dependency)| {
                dependency
                    .detail()
                    .and_then(|detail| detail.package.as_ref())
                    .unwrap_or(key)
                    .clone()
            })
            .filter(|name| !ignored_names.contains(name.as_str()))
            .collect();

        Ok(workspace_deps.difference(all_package_deps).cloned().collect())
    }

    pub fn get_relative_path(&self, manifest_path: &Path, workspace_root: &Path) -> PathBuf {
        let dir = manifest_path
            .parent()
            .unwrap_or(manifest_path);

        let current_dir = env::current_dir().unwrap_or_default();

        manifest_path
            .strip_prefix(&current_dir)
            .or_else(|_| manifest_path.strip_prefix(workspace_root))
            .unwrap_or(dir)
            .to_path_buf()
    }

    fn build_dependency_map(
        &self,
        deps: &[cargo_metadata::NodeDep],
        ignored_names: &HashSet<&str>,
    ) -> Result<HashMap<String, String>> {
        Ok(deps.iter()
            .map(|node_dep| {
                DependencyAnalyzer::parse_package_id(&node_dep.pkg.repr)
                    .map(|package_name| (node_dep.name.clone(), package_name))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|(_, name)| !ignored_names.contains(name.as_str()))
            .collect::<HashMap<_, _>>())
    }
}