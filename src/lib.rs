mod import_collector;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use bpaf::Bpaf;
use cargo_metadata::{Dependency, Metadata, MetadataCommand, Package};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::WalkDir;

use crate::import_collector::collect_imports;

// options("shear") + the "batteries" feature will strip name using `bpaf::cargo_helper` from `cargo shear"
// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options("shear"))]
pub struct CargoShearOptions {
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

    pub fn run(self) {
        self.shear();
    }

    fn shear(&self) {
        let metadata = MetadataCommand::new().current_dir(&self.options.path).exec().unwrap();
        let workspace_root = metadata.workspace_root.as_std_path();

        for package in metadata.workspace_packages() {
            Self::shear_package(workspace_root, package);
        }

        Self::shear_workspace(&metadata);
    }

    fn shear_workspace(metadata: &Metadata) {
        if metadata.workspace_packages().len() <= 1 {
            return;
        }
        let root_metadata_path = metadata.workspace_root.as_std_path();
        let root_metadata =
            cargo_toml::Manifest::from_path(root_metadata_path.join("Cargo.toml")).unwrap();
        let Some(workspace) = &root_metadata.workspace else { return };

        let all_package_deps = metadata
            .workspace_packages()
            .iter()
            .flat_map(|package| &package.dependencies)
            .map(Self::dependency_name)
            .collect::<Deps>();
        let workspace_deps = workspace.dependencies.keys().cloned().collect::<HashSet<String>>();
        let unused_workspace_deps = workspace_deps.difference(&all_package_deps);

        if !workspace_deps.is_empty() {
            println!("root: {unused_workspace_deps:?}");
        }
    }

    fn shear_package(workspace_root: &Path, package: &Package) {
        let dir = package.manifest_path.parent().unwrap().as_std_path();

        let rust_file_paths = package
            .targets
            .iter()
            .flat_map(|target| {
                if target.kind.iter().any(|s| s == "custom-build") {
                    vec![target.src_path.clone().into_std_path_buf()]
                } else {
                    let target_dir = target.src_path.parent().unwrap();
                    WalkDir::new(target_dir)
                        .into_iter()
                        .filter_map(std::result::Result::ok)
                        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
                        .map(walkdir::DirEntry::into_path)
                        .collect::<Vec<_>>()
                }
            })
            .collect::<HashSet<_>>();

        let rust_file_deps = rust_file_paths
            .par_iter()
            .filter_map(|path| Self::process_rust_source(path))
            .collect::<Vec<_>>();
        let rust_file_deps = rust_file_deps
            .into_iter()
            .reduce(|a, b| a.union(&b).cloned().collect())
            .unwrap_or_default();

        let package_deps_map = package
            .dependencies
            .iter()
            .map(|d| {
                let dep_name = Self::dependency_name(d);
                // change `package-name` and `Package_name` to `package_name`
                let mod_name = dep_name.clone().replace('-', "_").to_lowercase();
                (mod_name, dep_name)
            })
            .collect::<HashMap<String, String>>();

        let mod_names = package_deps_map.keys().cloned().collect::<HashSet<_>>();
        let unused_deps = mod_names.difference(&rust_file_deps).collect::<Vec<_>>();

        if !unused_deps.is_empty() {
            let unused_dep_names = unused_deps
                .into_iter()
                .map(|name| package_deps_map[name].clone())
                .collect::<Vec<_>>();
            println!("{:?}: {unused_dep_names:?}", dir.strip_prefix(workspace_root).unwrap());
        }
    }

    fn dependency_name(dependency: &Dependency) -> String {
        dependency.rename.as_ref().unwrap_or(&dependency.name).clone()
    }

    fn process_rust_source(path: &Path) -> Option<Deps> {
        let source_text = fs::read_to_string(path).unwrap();
        collect_imports(&source_text).ok()
    }
}
