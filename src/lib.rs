mod import_collector;

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, Result};
use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package};
use cargo_util_schemas::core::PackageIdSpec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::{DirEntry, WalkDir};

use crate::import_collector::collect_imports;

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

    unused_dependencies: AtomicUsize,
}

type Deps = HashSet<String>;

impl CargoShear {
    #[must_use]
    pub fn new(options: CargoShearOptions) -> Self {
        Self { options, unused_dependencies: AtomicUsize::default() }
    }

    #[must_use]
    pub fn run(self) -> ExitCode {
        println!("Analyzing {}", self.options.path.to_string_lossy());

        match self.shear() {
            Ok(()) => {
                let has_deps = self.unused_dependencies.load(Ordering::SeqCst) > 0;

                println!("Done!");

                if has_deps {
                    println!(
                        "\n\
                        If you believe cargo-shear has detected an unused dependency incorrectly,\n\
                        you can add the dependency to the list of dependencies to ignore in the\n\
                        `[package.metadata.cargo-shear]` section of the appropriate Cargo.toml.\n\
                        \n\
                        For example:\n\
                        \n\
                        [package.metadata.cargo-shear]\n\
                        ignored = [\"crate-name\"]"
                    );
                }

                // returns 0 if no deps, 1 if has deps
                ExitCode::from(u8::from(has_deps))
            }
            Err(err) => {
                println!("{err}");
                ExitCode::from(2)
            }
        }
    }

    fn shear(&self) -> Result<()> {
        let metadata = MetadataCommand::new()
            .features(CargoOpt::AllFeatures)
            .current_dir(&self.options.path)
            .exec()?;

        let package_dependencies = metadata
            .workspace_packages()
            .par_iter()
            .map(|package| self.shear_package(&metadata, package))
            .collect::<Result<Vec<Deps>>>()?
            .into_iter()
            .fold(HashSet::new(), |a, b| a.union(&b).cloned().collect());

        self.shear_workspace(&metadata, &package_dependencies)
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

        let path = cargo_toml_path
            .strip_prefix(env::current_dir()?)
            .unwrap_or(&cargo_toml_path)
            .to_string_lossy();
        println!("root -- {path}:",);
        for unused_dep in &unused_deps {
            println!("  {unused_dep}");
        }
        println!();
        self.try_fix_package(&cargo_toml_path, &unused_deps)?;
        self.unused_dependencies.fetch_add(unused_deps.len(), Ordering::SeqCst);
        Ok(())
    }

    /// Returns the remaining package dependency names.
    fn shear_package(&self, metadata: &Metadata, package: &Package) -> Result<Deps> {
        let workspace_root = metadata.workspace_root.as_std_path();

        let dir = package
            .manifest_path
            .parent()
            .unwrap_or_else(|| panic!("failed to get parent path {}", &package.manifest_path))
            .as_std_path();

        let ignored_package_names = Self::get_ignored_package_names(package);

        let package_dependency_names_map = metadata
            .resolve
            .as_ref()
            .context("`cargo_metadata::MetadataCommand::no_deps` should not be called.")?
            .nodes
            .iter()
            .find(|node| node.id == package.id)
            .context("package should exist")?
            .deps // `deps` handles renamed dependencies whereas `dependencies` does not
            .iter()
            .filter_map(|node_dep| {
                let package_name =
                    PackageIdSpec::parse(&node_dep.pkg.repr).ok()?.name().to_string();
                Some((node_dep.name.clone(), package_name))
            })
            .filter(|(_, name)| !ignored_package_names.contains(name.as_str()))
            .collect::<HashMap<String, String>>();

        let module_names_from_package_deps =
            package_dependency_names_map.keys().cloned().collect::<HashSet<_>>();

        let package_dependency_names =
            package_dependency_names_map.values().cloned().collect::<HashSet<_>>();

        let module_names_from_rust_files = Self::get_package_dependencies_from_rust_files(package)?;

        let unused_module_names = module_names_from_package_deps
            .difference(&module_names_from_rust_files)
            .collect::<Vec<_>>();

        if unused_module_names.is_empty() {
            return Ok(package_dependency_names);
        }

        let unused_dependency_names = unused_module_names
            .into_iter()
            .map(|name| package_dependency_names_map[name].clone())
            .collect::<Vec<_>>();

        self.try_fix_package(package.manifest_path.as_std_path(), &unused_dependency_names)?;

        if !unused_dependency_names.is_empty() {
            self.unused_dependencies.fetch_add(unused_dependency_names.len(), Ordering::SeqCst);
            let name = &package.name;
            let path = package
                .manifest_path
                .as_std_path()
                .strip_prefix(workspace_root)
                .unwrap_or(dir)
                .to_string_lossy();
            println!("{name} -- {path}:");
            for unused_dep in &unused_dependency_names {
                println!("  {unused_dep}");
            }
            println!();
        }

        let package_dependency_names = package_dependency_names
            .difference(&HashSet::from_iter(unused_dependency_names))
            .cloned()
            .collect::<Deps>();
        Ok(package_dependency_names)
    }

    fn get_ignored_package_names(package: &Package) -> HashSet<&str> {
        package
            .metadata
            .as_object()
            .and_then(|object| object.get("cargo-shear"))
            .and_then(|object| object.get("ignored"))
            .and_then(|ignored| ignored.as_array())
            .map(|ignored| ignored.iter().filter_map(|item| item.as_str()).collect::<HashSet<_>>())
            .unwrap_or_default()
    }

    fn get_package_dependencies_from_rust_files(package: &Package) -> Result<Deps> {
        Ok(Self::get_package_rust_files(package)
            .par_iter()
            .map(|path| Self::process_rust_source(path))
            .collect::<Result<Vec<Deps>>>()?
            .into_iter()
            .fold(HashSet::new(), |a, b| a.union(&b).cloned().collect()))
    }

    fn get_package_rust_files(package: &Package) -> Vec<PathBuf> {
        package
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
            .collect()
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

        // Try `[workspace.dependencies]`
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

        // Try `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
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
        fs::write(cargo_toml_path, serialized)?;
        Ok(())
    }
}
