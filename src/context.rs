//! Context types for processing workspaces and packages.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use cargo_metadata::{Metadata, Package, Target};
use ignore::{DirEntry, WalkBuilder}; // Changed from walkdir
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{manifest::Manifest, source_parser::ParsedSource, util::read_to_string};

/// Context for processing a workspace.
pub struct WorkspaceContext {
    /// Workspace root path.
    pub root: PathBuf,

    /// Absolute path to `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// Raw manifest content.
    pub manifest_content: String,
    /// The workspace manifest.
    pub manifest: Manifest,

    /// Package path to source file map.
    pub files: FxHashMap<PathBuf, ParsedSource>,
    /// All linked files.
    pub linked: FxHashSet<PathBuf>,
    /// Paths of all packages in the workspace.
    pub packages: FxHashSet<PathBuf>,

    /// Mapping from dependency key to package name.
    pub dep_to_pkg: FxHashMap<String, String>,
    /// Ignored dependency keys.
    pub ignored_deps: FxHashSet<String>,
}

impl WorkspaceContext {
    pub fn new(metadata: &Metadata) -> Result<Self> {
        let root = metadata.workspace_root.as_std_path().to_path_buf();

        let manifest_path = root.join("Cargo.toml");
        let manifest_content = read_to_string(&manifest_path)?;
        let manifest: Manifest = toml::from_str(&manifest_content)?;

        let packages: FxHashSet<PathBuf> = metadata
            .workspace_packages()
            .iter()
            .filter_map(|pkg| pkg.manifest_path.parent())
            .map(|path| path.as_std_path().to_path_buf())
            .collect();

        let entry_points: FxHashSet<PathBuf> = metadata
            .workspace_packages()
            .iter()
            .flat_map(|pkg| pkg.targets.iter())
            .map(|target| target.src_path.as_std_path().to_path_buf())
            .collect();

        // Get parent dirs of entry points
        let parents: FxHashSet<PathBuf> =
            entry_points.iter().filter_map(|path| path.parent()).map(Path::to_path_buf).collect();

        let walked: FxHashSet<PathBuf> = parents
            .into_par_iter()
            .flat_map_iter(|parent| {
                let packages = packages.clone();
                WalkBuilder::new(&parent)
                    // Skip nested packages (but allow package roots)
                    .filter_entry(move |entry| {
                        if let Some(file_type) = entry.file_type()
                            && file_type.is_dir()
                        {
                            let path = entry.path();
                            if path.join("Cargo.toml").exists() {
                                return packages.contains(path);
                            }
                        }

                        true
                    })
                    .build()
                    .filter_map(Result::ok)
                    // Only process Rust files
                    .filter(|entry| {
                        entry.file_type().is_some_and(|file_type| file_type.is_file())
                            && entry.path().extension().is_some_and(|extension| extension == "rs")
                    })
                    .map(DirEntry::into_path)
            })
            .collect();

        // Include single file entry points (build.rs)
        let paths: FxHashSet<PathBuf> =
            entry_points.iter().filter(|path| path.is_file()).cloned().chain(walked).collect();

        let files: FxHashMap<PathBuf, ParsedSource> = paths
            .into_par_iter()
            .filter_map(|path| {
                let is_entry_point = entry_points.contains(&path);
                ParsedSource::from_path(&path, is_entry_point).ok().map(|source| (path, source))
            })
            .collect();

        let linked: FxHashSet<PathBuf> = entry_points
            .into_iter()
            .chain(files.values().flat_map(|parsed| {
                parsed.paths.iter().filter_map(|path| {
                    // Only canonicalize if needed.
                    if path.as_os_str().as_encoded_bytes().starts_with(b".") {
                        path.canonicalize().ok()
                    } else {
                        Some(path.clone())
                    }
                })
            }))
            .collect();

        let dep_to_pkg = manifest
            .workspace
            .dependencies
            .iter()
            .map(|(dep, dependency)| {
                let dep = dep.get_ref();
                let pkg = dependency.get_ref().package().unwrap_or(dep);
                (dep.to_owned(), pkg.to_owned())
            })
            .collect();

        let ignored_deps = manifest
            .workspace
            .metadata
            .cargo_shear
            .ignored
            .iter()
            .map(|ignore| ignore.get_ref().clone())
            .collect();

        Ok(Self {
            root,
            manifest_path,
            manifest_content,
            manifest,
            files,
            linked,
            packages,
            dep_to_pkg,
            ignored_deps,
        })
    }
}

/// Context for processing a package.
pub struct PackageContext<'a> {
    /// The workspace context.
    pub workspace: &'a WorkspaceContext,

    /// Package name.
    pub name: String,
    /// Package directory.
    pub directory: PathBuf,

    /// Absolute path to `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// Raw manifest content.
    pub manifest_content: String,
    /// Package manifest.
    pub manifest: Manifest,
    /// Cargo targets for this package.
    pub targets: Vec<Target>,

    /// Mapping from import name to package name.
    pub import_to_pkg: FxHashMap<String, String>,
    /// Mapping from package name to import name.
    pub pkg_to_import: FxHashMap<String, String>,

    /// Ignored import names.
    pub ignored_imports: FxHashSet<String>,
}

impl<'a> PackageContext<'a> {
    pub fn new(
        workspace: &'a WorkspaceContext,
        package: &Package,
        metadata: &Metadata,
    ) -> Result<Self> {
        let manifest_path = package.manifest_path.as_std_path();
        let manifest_content = read_to_string(manifest_path)?;
        let manifest: Manifest = toml::from_str(&manifest_content)?;

        let resolved = metadata
            .resolve
            .as_ref()
            .ok_or_else(|| {
                anyhow!("`cargo_metadata::MetadataCommand::no_deps` should not be called.")
            })?
            .nodes
            .iter()
            .find(|node| node.id == package.id)
            .ok_or_else(|| anyhow!("Package not found: {}", package.name))?;

        let directory = manifest_path
            .parent()
            .ok_or_else(|| anyhow!("Package has no parent directory: {}", package.name))?
            .to_path_buf();

        let mut import_to_pkg = FxHashMap::default();
        let mut pkg_to_import = FxHashMap::default();

        for dep in &resolved.deps {
            if let Some(pkg) = metadata.packages.iter().find(|package| package.id == dep.pkg) {
                import_to_pkg.insert(dep.name.clone(), pkg.name.to_string());
                pkg_to_import.insert(pkg.name.to_string(), dep.name.clone());
            }
        }

        let package_ignored_deps = &manifest.package.metadata.cargo_shear.ignored;
        let workspace_ignored_deps = &workspace.manifest.workspace.metadata.cargo_shear.ignored;
        let ignored_imports = package_ignored_deps
            .iter()
            .chain(workspace_ignored_deps)
            .map(|dep| dep.get_ref().replace('-', "_"))
            .collect();

        Ok(Self {
            workspace,
            name: package.name.to_string(),
            directory,
            manifest_path: manifest_path.to_path_buf(),
            manifest_content,
            manifest,
            targets: package.targets.clone(),
            import_to_pkg,
            pkg_to_import,
            ignored_imports,
        })
    }
}
