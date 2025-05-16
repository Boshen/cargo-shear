mod import_collector;
#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    str::FromStr,
};

use anyhow::{Context, Result};
use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Package, TargetKind};
use cargo_util_schemas::core::PackageIdSpec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::{DirEntry, WalkDir};

use crate::import_collector::collect_imports;

const VERSION: &str = match option_env!("SHEAR_VERSION") {
    Some(v) => v,
    None => "dev",
};

// options("shear") + the "batteries" feature will strip name using `bpaf::cargo_helper` from `cargo shear"
// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options("shear"), version(VERSION))]
pub struct CargoShearOptions {
    /// Remove unused dependencies.
    #[bpaf(long)]
    fix: bool,

    /// Uses `cargo expand` to expand macros, which requires nightly and is significantly slower.
    #[bpaf(long)]
    expand: bool,

    /// Package(s) to check
    /// If not specified, all packages are checked by default
    #[bpaf(long, short, argument("SPEC"))]
    package: Vec<String>,

    /// Exclude packages from the check
    exclude: Vec<String>,

    #[bpaf(positional("PATH"), fallback_with(default_path))]
    path: PathBuf,
}

pub(crate) fn default_path() -> Result<PathBuf> {
    env::current_dir().map_err(|err| anyhow::anyhow!(err))
}

pub struct CargoShear {
    options: CargoShearOptions,

    unused_dependencies: usize,

    fixed_dependencies: usize,
}

type Deps = HashSet<String>;

impl CargoShear {
    #[must_use]
    pub const fn new(options: CargoShearOptions) -> Self {
        Self { options, unused_dependencies: 0, fixed_dependencies: 0 }
    }

    #[must_use]
    pub fn run(mut self) -> ExitCode {
        println!("Analyzing {}", self.options.path.to_string_lossy());
        println!();

        match self.shear() {
            Ok(()) => {
                let has_fixed = self.fixed_dependencies > 0;

                if has_fixed {
                    println!("Fixed {} dependencies!", self.fixed_dependencies);
                }

                let has_deps = (self.unused_dependencies - self.fixed_dependencies) > 0;

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
                } else {
                    println!("No unused dependencies!");
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

    fn shear(&mut self) -> Result<()> {
        let metadata = MetadataCommand::new()
            .features(CargoOpt::AllFeatures)
            .current_dir(&self.options.path)
            .exec()?;

        let mut package_dependencies = HashSet::new();
        for package in metadata.workspace_packages() {
            // Skip if package is in the exclude list
            if self.options.exclude.iter().any(|name| name == &package.name) {
                continue;
            }

            // Skip if specific packages are specified and this package is not in the list
            if !self.options.package.is_empty()
                && !self.options.package.iter().any(|name| name == &package.name)
            {
                continue;
            }

            let deps = self.shear_package(&metadata, package)?;
            package_dependencies.extend(deps);
        }

        self.shear_workspace(&metadata, &package_dependencies)
    }

    fn shear_workspace(
        &mut self,
        workspace_metadata: &Metadata,
        all_pkg_deps: &Deps,
    ) -> Result<()> {
        if workspace_metadata.workspace_packages().len() <= 1 {
            return Ok(());
        }
        let metadata_path = workspace_metadata.workspace_root.as_std_path();
        let cargo_toml_path = metadata_path.join("Cargo.toml");
        let metadata = cargo_toml::Manifest::from_path(&cargo_toml_path)?;
        let Some(workspace) = &metadata.workspace else { return Ok(()) };

        let ignored_package_names =
            Self::get_ignored_package_names(&workspace_metadata.workspace_metadata);

        let workspace_deps = workspace
            .dependencies
            .iter()
            .map(|(key, dependency)| {
                // renamed package, e.g. `ustr = { package = "ustr-fxhash", version = "1.0.0" }`
                dependency
                    .detail()
                    .and_then(|detail| detail.package.as_ref())
                    .unwrap_or(key)
                    .clone()
            })
            .filter(|name| !ignored_package_names.contains(name.as_str()))
            .collect::<HashSet<_>>();

        let unused_deps = workspace_deps.difference(all_pkg_deps).cloned().collect::<HashSet<_>>();

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
        self.unused_dependencies += unused_deps.len();
        Ok(())
    }

    /// Returns the remaining package dependency names.
    fn shear_package(&mut self, metadata: &Metadata, package: &Package) -> Result<Deps> {
        let workspace_root = metadata.workspace_root.as_std_path();
        let dir = package
            .manifest_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("failed to get parent path {}", &package.manifest_path))?
            .as_std_path();
        let relative_path = package
            .manifest_path
            .as_std_path()
            .strip_prefix(workspace_root)
            .unwrap_or(dir)
            .to_string_lossy();

        let mut ignored_package_names = Self::get_ignored_package_names(&package.metadata);
        ignored_package_names.extend(Self::get_ignored_package_names(&metadata.workspace_metadata));

        let this_package = metadata
            .resolve
            .as_ref()
            .context("`cargo_metadata::MetadataCommand::no_deps` should not be called.")?
            .nodes
            .iter()
            .find(|node| node.id == package.id)
            .context("package should exist")?;

        let package_dependency_names_map = this_package
            .deps // `deps` handles renamed dependencies whereas `dependencies` does not
            .iter()
            .map(|node_dep| {
                Self::parse_package_id(&node_dep.pkg.repr)
                    .map(|package_name| (node_dep.name.clone(), package_name))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|(_, name)| !ignored_package_names.contains(name.as_str()))
            .collect::<HashMap<String, String>>();

        let module_names_from_package_deps =
            package_dependency_names_map.keys().cloned().collect::<HashSet<_>>();

        let package_dependency_names =
            package_dependency_names_map.values().cloned().collect::<HashSet<_>>();

        let module_names_from_rust_files = if self.options.expand {
            Self::get_package_dependencies_from_expand(package)
        } else {
            Self::get_package_dependencies_from_rust_files(package)
        }?;

        let unused_module_names = module_names_from_package_deps
            .difference(&module_names_from_rust_files)
            .collect::<Vec<_>>();

        if unused_module_names.is_empty() {
            return Ok(package_dependency_names);
        }

        let unused_dependency_names = unused_module_names
            .into_iter()
            .map(|name| package_dependency_names_map[name].clone())
            .collect::<HashSet<_>>();

        self.try_fix_package(package.manifest_path.as_std_path(), &unused_dependency_names)?;

        if !unused_dependency_names.is_empty() {
            self.unused_dependencies += unused_dependency_names.len();
            println!("{} -- {relative_path}:", package.name);
            for unused_dep in &unused_dependency_names {
                println!("  {unused_dep}");
            }
            println!();
        }

        let package_dependency_names = package_dependency_names
            .difference(&unused_dependency_names)
            .cloned()
            .collect::<Deps>();
        Ok(package_dependency_names)
    }

    fn parse_package_id(s: &str) -> Result<String> {
        // The node id can have multiple representations:
        if s.contains(' ') {
            // < Rust 1.77 : `memchr 2.7.1 (registry+https://github.com/rust-lang/crates.io-index)`
            s.split(' ')
                .next()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow::anyhow!("{s} should have a space"))
        } else {
            // >= Rust 1.77: `registry+https://github.com/rust-lang/crates.io-index#memchr@2.7.1`
            PackageIdSpec::parse(s)
                .map(|id| id.name().to_owned())
                .map_err(|err| anyhow::anyhow!(err))
        }
    }

    fn get_ignored_package_names(value: &serde_json::Value) -> HashSet<&str> {
        value
            .as_object()
            .and_then(|object| object.get("cargo-shear"))
            .and_then(|object| object.get("ignored"))
            .and_then(|ignored| ignored.as_array())
            .map(|ignored| ignored.iter().filter_map(|item| item.as_str()).collect::<HashSet<_>>())
            .unwrap_or_default()
    }

    fn get_package_dependencies_from_expand(package: &Package) -> Result<Deps> {
        // Unfortunately, cargo expand alone isn't enough to get all dependencies.
        // ex. #[async_trait::async_trait] is removed when expanded and looks unused.
        let mut combined_imports = Self::get_package_dependencies_from_rust_files(package)?;

        for target in &package.targets {
            let target_arg = match target.kind.first().context("Failed to get `target`.")? {
                TargetKind::CustomBuild => continue, // Handled by `get_package_rust_files`
                TargetKind::Bin => format!("--bin={}", target.name),
                TargetKind::Example => format!("--example={}", target.name),
                TargetKind::Test => format!("--test={}", target.name),
                TargetKind::Bench => format!("--bench={}", target.name),
                TargetKind::CDyLib
                | TargetKind::DyLib
                | TargetKind::Lib
                | TargetKind::ProcMacro
                | TargetKind::RLib
                | TargetKind::StaticLib
                | TargetKind::Unknown(_)
                | _ => "--lib".to_owned(),
            };

            let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

            // Use `cargo rustc` to invoke rustc directly with -Zunpretty=expanded
            let mut cmd = Command::new(cargo);
            cmd.arg("rustc");
            cmd.arg(&target_arg);
            cmd.arg("--all-features");
            cmd.arg("--profile=check"); // or release/test depending on your needs
            cmd.arg("--color=never");
            cmd.arg("--");
            cmd.arg("-Zunpretty=expanded");
            cmd.current_dir(package.manifest_path.parent().context("Failed to get parent dir.")?);

            let output = cmd.output()?;
            if !output.status.success() {
                anyhow::bail!(
                    "cargo expand failed ({})\n{:?}\n{}",
                    output.status,
                    cmd,
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let output_str = String::from_utf8(output.stdout)?;
            if output_str.is_empty() {
                anyhow::bail!("cargo expand returned empty output");
            }

            let imports = collect_imports(&output_str)?;
            combined_imports.extend(imports);
        }

        Ok(combined_imports)
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
                if target.kind.contains(&TargetKind::CustomBuild) {
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

    #[expect(clippy::option_if_let_else, reason = "Current code is more readable.")]
    fn try_fix_package(
        &mut self,
        cargo_toml_path: &Path,
        unused_dep_names: &HashSet<String>,
    ) -> Result<()> {
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
            dependencies.retain(|k, _| !unused_dep_names.contains(k));
        }

        // Try `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`
        for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) =
                manifest.get_mut(table_name).and_then(|item| item.as_table_mut())
            {
                dependencies.retain(|k, _| !unused_dep_names.contains(k));
            }
        }

        // Fix any features that refer to the removed dependencies.
        //
        // Before:
        //  [features]
        //  default = ["dep1", "dep2"]
        //  other-feature = ["dep:dep1", "dep3"]

        // After:
        //  [features]
        //  default = ["dep2"]
        //  other-feature = ["dep3"]
        if let Some(features) = manifest.get_mut("features").and_then(|item| item.as_table_mut()) {
            for (_feature_name, dependencies) in features.iter_mut() {
                if let Some(dependencies) = dependencies.as_array_mut() {
                    dependencies.retain(|dep| {
                        match dep.as_str() {
                            Some(dep) => {
                                // Check if the dep: prefix is present.
                                match dep.strip_prefix("dep:") {
                                    // The dependency has a dep:prefix, so it's an explicit dependency we can remove.
                                    Some(dep) => !unused_dep_names.contains(dep),
                                    // This is slightly incorrect, as it will remove any features with the same name as the removed dependency.
                                    // It's not clear what we should do here, as it maybe we should remove the implicit feature entirely?
                                    // Or maybe the feature is just poorly named?
                                    // Either way, do the simple thing and let `cargo check` complain if we goofed.
                                    None => !unused_dep_names.contains(dep),
                                }
                            }
                            None => true,
                        }
                    });
                }
            }
        }

        self.fixed_dependencies += unused_dep_names.len();
        let serialized = manifest.to_string();
        fs::write(cargo_toml_path, serialized)?;
        Ok(())
    }
}
