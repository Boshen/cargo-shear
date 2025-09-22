use std::collections::HashSet;
use std::ffi::OsString;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::{Package, TargetKind};
use cargo_util_schemas::core::PackageIdSpec;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use walkdir::{DirEntry, WalkDir};

use crate::error::{Error, Result};
use crate::import_collector::collect_imports;

pub type Dependencies = HashSet<String>;

pub struct DependencyAnalyzer {
    expand_macros: bool,
}

impl DependencyAnalyzer {
    pub fn new(expand_macros: bool) -> Self {
        Self { expand_macros }
    }

    pub fn analyze_package(&self, package: &Package) -> Result<Dependencies> {
        if self.expand_macros {
            self.analyze_with_expansion(package)
        } else {
            self.analyze_from_files(package)
        }
    }

    fn analyze_from_files(&self, package: &Package) -> Result<Dependencies> {
        let rust_files = self.get_package_rust_files(package);

        let deps_vec: Vec<Dependencies> = rust_files
            .par_iter()
            .map(|path| self.process_rust_source(path))
            .collect::<Result<Vec<_>>>()?;

        Ok(deps_vec.into_iter().fold(HashSet::new(), |a, b| {
            a.union(&b).cloned().collect()
        }))
    }

    fn analyze_with_expansion(&self, package: &Package) -> Result<Dependencies> {
        let mut combined_imports = self.analyze_from_files(package)?;

        for target in &package.targets {
            let target_arg = match target.kind.first().ok_or_else(|| {
                Error::metadata("Failed to get target kind".to_string())
            })? {
                TargetKind::CustomBuild => continue,
                TargetKind::Bin => format!("--bin={}", target.name),
                TargetKind::Example => format!("--example={}", target.name),
                TargetKind::Test => format!("--test={}", target.name),
                TargetKind::Bench => format!("--bench={}", target.name),
                _ => "--lib".to_owned(),
            };

            let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

            let mut cmd = Command::new(cargo);
            cmd.arg("rustc")
                .arg(&target_arg)
                .arg("--all-features")
                .arg("--profile=check")
                .arg("--color=never")
                .arg("--")
                .arg("-Zunpretty=expanded")
                .current_dir(
                    package.manifest_path.parent()
                        .ok_or_else(|| Error::missing_parent(package.manifest_path.clone().into()))?
                );

            let output = cmd.output()?;
            if !output.status.success() {
                return Err(Error::expand(
                    target.name.clone(),
                    String::from_utf8_lossy(&output.stderr).to_string(),
                ));
            }

            let output_str = String::from_utf8(output.stdout)?;
            if output_str.is_empty() {
                return Err(Error::expand(
                    target.name.clone(),
                    "Empty output from cargo expand".to_string(),
                ));
            }

            let imports = collect_imports(&output_str)?;
            combined_imports.extend(imports);
        }

        Ok(combined_imports)
    }

    fn get_package_rust_files(&self, package: &Package) -> Vec<PathBuf> {
        package.targets
            .iter()
            .flat_map(|target| {
                if target.kind.contains(&TargetKind::CustomBuild) {
                    vec![target.src_path.clone().into_std_path_buf()]
                } else {
                    let target_dir = target.src_path.parent()
                        .expect(&format!("Failed to get parent path {}", &target.src_path));

                    WalkDir::new(target_dir)
                        .into_iter()
                        .filter_map(std::result::Result::ok)
                        .filter(|e| {
                            e.file_type().is_file()
                                && e.path().extension().is_some_and(|ext| ext == "rs")
                        })
                        .map(DirEntry::into_path)
                        .collect::<Vec<_>>()
                }
            })
            .collect()
    }

    fn process_rust_source(&self, path: &Path) -> Result<Dependencies> {
        let source_text = std::fs::read_to_string(path)
            .map_err(|e| Error::io(e))?;

        collect_imports(&source_text)
            .map_err(|e| e.into())
    }

    pub fn parse_package_id(s: &str) -> Result<String> {
        if s.contains(' ') {
            s.split(' ')
                .next()
                .map(ToString::to_string)
                .ok_or_else(|| Error::parse(format!("{} should have a space", s)))
        } else {
            PackageIdSpec::parse(s)
                .map(|id| id.name().to_owned())
                .map_err(|e| Error::parse(e.to_string()))
        }
    }

    pub fn get_ignored_package_names(value: &serde_json::Value) -> HashSet<&str> {
        value
            .as_object()
            .and_then(|object| object.get("cargo-shear"))
            .and_then(|object| object.get("ignored"))
            .and_then(|ignored| ignored.as_array())
            .map(|ignored| {
                ignored.iter()
                    .filter_map(|item| item.as_str())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default()
    }
}