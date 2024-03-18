mod import_collector;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use bpaf::Bpaf;
use cargo_metadata::{Dependency, Metadata, MetadataCommand, Package};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::{DirEntry, WalkDir};

use crate::import_collector::collect_imports;

#[derive(Debug)]
pub struct Error;

// options("shear") + the "batteries" feature will strip name using `bpaf::cargo_helper` from `cargo shear"
// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options("shear"))]
pub struct CargoShearOptions {
    #[bpaf(long)]
    fix: bool,

    #[bpaf(positional("PATH"), fallback(PathBuf::from(".")))]
    path: PathBuf,
}

pub struct CargoShear {
    options: CargoShearOptions,
}

type Deps = HashSet<String>;

impl CargoShear {
    #[must_use]
    pub fn new(options: CargoShearOptions) -> Self {
        Self { options }
    }

    /// # Errors
    pub fn run(self) -> Result<()> {
        self.shear()
    }

    fn shear(&self) -> Result<()> {
        let metadata = MetadataCommand::new().current_dir(&self.options.path).exec()?;
        let workspace_root = metadata.workspace_root.as_std_path();

        let all_pkg_deps = metadata
            .workspace_packages()
            .par_iter()
            .map(|package| self.shear_package(workspace_root, package))
            .collect::<Result<Vec<Deps>>>()?
            .into_iter()
            .fold(HashSet::new(), |a, b| a.union(&b).cloned().collect());

        self.shear_workspace(&metadata, &all_pkg_deps)
    }

    fn shear_workspace(&self, metadata: &Metadata, all_pkg_deps: &Deps) -> Result<()> {
        if metadata.workspace_packages().len() <= 1 {
            return Ok(());
        }
        let metadata_path = metadata.workspace_root.as_std_path();
        let cargo_toml_path = metadata_path.join("Cargo.toml");
        let metadata = cargo_toml::Manifest::from_path(&cargo_toml_path)?;
        let Some(workspace) = &metadata.workspace else { return Ok(()) };

        let workspace_deps = workspace.dependencies.keys().cloned().collect::<HashSet<String>>();
        let unused_deps = workspace_deps.difference(all_pkg_deps).cloned().collect::<Vec<_>>();

        if unused_deps.is_empty() {
            return Ok(());
        }
        println!("root: {unused_deps:?}");
        self.try_fix_package(&cargo_toml_path, &unused_deps)
    }

    /// Returns the remaining dependencies
    fn shear_package(&self, workspace_root: &Path, package: &Package) -> Result<Deps> {
        let dir = package
            .manifest_path
            .parent()
            .unwrap_or_else(|| panic!("failed to get parent path {}", &package.manifest_path))
            .as_std_path();

        let rust_file_paths = package
            .targets
            .iter()
            .flat_map(|target| {
                if target.kind.iter().any(|s| s == "custom-build") {
                    vec![target.src_path.clone().into_std_path_buf()]
                } else {
                    let target_dir = target.src_path.parent().unwrap_or_else(|| {
                        panic!("failed to get parentp path {}", &target.src_path)
                    });
                    WalkDir::new(target_dir)
                        .into_iter()
                        .filter_map(Result::ok)
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
                        .map(DirEntry::into_path)
                        .collect::<Vec<_>>()
                }
            })
            .collect::<HashSet<_>>();

        let rust_file_deps = rust_file_paths
            .par_iter()
            .map(|path| Self::process_rust_source(path))
            .collect::<Result<Vec<Deps>>>()?
            .into_iter()
            .fold(HashSet::new(), |a, b| a.union(&b).cloned().collect());

        let dependency_names =
            package.dependencies.iter().map(Self::dependency_name).collect::<Deps>();

        let package_deps_map = dependency_names
            .iter()
            .map(|dep_name| {
                // change `package-name` and `Package_name` to `package_name`
                let mod_name = dep_name.clone().replace('-', "_").to_lowercase();
                (mod_name, dep_name.clone())
            })
            .collect::<HashMap<String, String>>();

        let mod_names = package_deps_map.keys().cloned().collect::<HashSet<_>>();
        let unused_deps = mod_names.difference(&rust_file_deps).collect::<Vec<_>>();

        if unused_deps.is_empty() {
            return Ok(dependency_names);
        }

        let unused_dep_names =
            unused_deps.into_iter().map(|name| package_deps_map[name].clone()).collect::<Vec<_>>();
        let path = dir.strip_prefix(workspace_root).unwrap_or(dir);
        println!("{path:?}: {unused_dep_names:?}");
        self.try_fix_package(package.manifest_path.as_std_path(), &unused_dep_names)?;
        let dependency_names = dependency_names
            .difference(&HashSet::from_iter(unused_dep_names))
            .cloned()
            .collect::<Deps>();
        Ok(dependency_names)
    }

    fn dependency_name(dependency: &Dependency) -> String {
        dependency.rename.as_ref().unwrap_or(&dependency.name).clone()
    }

    fn process_rust_source(path: &Path) -> Result<Deps> {
        let source_text = fs::read_to_string(path)?;
        let imports = collect_imports(&source_text)?;
        Ok(imports)
    }

    fn try_fix_package(&self, cargo_toml_path: &Path, unused_dep_names: &[String]) -> Result<()> {
        if !self.options.fix {
            return Ok(());
        }

        let manifest = fs::read_to_string(cargo_toml_path)?;
        let mut manifest = toml_edit::DocumentMut::from_str(&manifest)?;

        if let Some(dependencies) = manifest
            .get_mut("workspace")
            .and_then(|item| item.as_table_mut())
            .and_then(|table| table.get_mut("dependencies"))
            .and_then(|item| item.as_table_mut())
        {
            for k in unused_dep_names {
                dependencies.remove(k);
            }
        }

        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) =
                manifest.get_mut(table_name).and_then(|item| item.as_table_mut())
            {
                for k in unused_dep_names {
                    dependencies.remove(k);
                }
            }
        }

        let serialized = manifest.to_string();
        fs::write(cargo_toml_path, serialized).expect("Cargo.toml write error");
        Ok(())
    }
}
