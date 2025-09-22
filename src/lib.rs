mod cargo_toml_editor;
mod dependency_analyzer;
mod error;
mod import_collector;
mod package_processor;
#[cfg(test)]
mod tests;

use std::{
    backtrace::BacktraceStatus,
    collections::HashSet,
    env,
    path::PathBuf,
    process::ExitCode,
};

use bpaf::Bpaf;
use cargo_metadata::{CargoOpt, MetadataCommand};

use crate::cargo_toml_editor::CargoTomlEditor;
use crate::error::{Error, Result};
use crate::package_processor::PackageProcessor;

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

impl CargoShearOptions {
    /// Create a new `CargoShearOptions` for testing purposes
    #[must_use]
    pub const fn new_for_test(path: PathBuf, fix: bool) -> Self {
        Self { fix, expand: false, package: vec![], exclude: vec![], path }
    }
}

pub(crate) fn default_path() -> Result<PathBuf> {
    env::current_dir().map_err(Error::io)
}

pub struct CargoShear {
    options: CargoShearOptions,

    unused_dependencies: usize,

    fixed_dependencies: usize,
}

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
                    println!(
                        "Fixed {} {}.\n",
                        self.fixed_dependencies,
                        if self.fixed_dependencies == 1 { "dependency" } else { "dependencies" }
                    );
                }

                let has_deps = (self.unused_dependencies - self.fixed_dependencies) > 0;

                if has_deps {
                    println!(
                        "\n\
                        cargo-shear may have detected unused dependencies incorrectly due to its limitations.\n\
                        They can be ignored by adding the crate name to the package's Cargo.toml:\n\n\
                        [package.metadata.cargo-shear]\n\
                        ignored = [\"crate-name\"]\n\n\
                        or in the workspace Cargo.toml:\n\n\
                        [workspace.metadata.cargo-shear]\n\
                        ignored = [\"crate-name\"]\n"
                    );
                } else {
                    println!("No unused dependencies!");
                }

                ExitCode::from(u8::from(if self.options.fix { has_fixed } else { has_deps }))
            }
            Err(err) => {
                println!("{err:?}");
                if err.backtrace().status() == BacktraceStatus::Disabled {
                    println!(
                        "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace"
                    );
                }
                ExitCode::from(2)
            }
        }
    }

    fn shear(&mut self) -> Result<()> {
        let metadata = MetadataCommand::new()
            .features(CargoOpt::AllFeatures)
            .current_dir(&self.options.path)
            .exec()
            .map_err(|e| Error::metadata(e.to_string()))?;

        let processor = PackageProcessor::new(self.options.expand);
        let mut package_dependencies = HashSet::new();

        for package in metadata.workspace_packages() {
            // Skip if package is in the exclude list
            if self.options.exclude.iter().any(|name| name == package.name.as_str()) {
                continue;
            }

            // Skip if specific packages are specified and this package is not in the list
            if !self.options.package.is_empty()
                && !self.options.package.iter().any(|name| name == package.name.as_str())
            {
                continue;
            }

            let result = processor.process_package(&metadata, package)?;

            if !result.unused_dependencies.is_empty() {
                let relative_path = processor.get_relative_path(
                    package.manifest_path.as_std_path(),
                    metadata.workspace_root.as_std_path(),
                );

                println!("{} -- {}:", package.name, relative_path.display());
                for unused_dep in &result.unused_dependencies {
                    println!("  {unused_dep}");
                }
                println!();

                self.unused_dependencies += result.unused_dependencies.len();

                if self.options.fix {
                    let fixed = CargoTomlEditor::remove_dependencies(
                        package.manifest_path.as_std_path(),
                        &result.unused_dependencies,
                    )?;
                    self.fixed_dependencies += fixed;
                }
            }

            package_dependencies.extend(result.remaining_dependencies);
        }

        // Process workspace dependencies
        let workspace_unused = processor.process_workspace(&metadata, &package_dependencies)?;

        if !workspace_unused.is_empty() {
            let cargo_toml_path = metadata.workspace_root.as_std_path().join("Cargo.toml");
            let path = cargo_toml_path
                .strip_prefix(env::current_dir().unwrap_or_default())
                .unwrap_or(&cargo_toml_path)
                .to_string_lossy();

            println!("root -- {path}:");
            for unused_dep in &workspace_unused {
                println!("  {unused_dep}");
            }
            println!();

            self.unused_dependencies += workspace_unused.len();

            if self.options.fix {
                let fixed = CargoTomlEditor::remove_dependencies(
                    &cargo_toml_path,
                    &workspace_unused,
                )?;
                self.fixed_dependencies += fixed;
            }
        }

        Ok(())
    }
}