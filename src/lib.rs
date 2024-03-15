use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use bpaf::Bpaf;
use cargo_metadata::{Metadata, MetadataCommand, Package};
use walkdir::WalkDir;

type Deps = HashSet<String>;

// options("shear") + the "batteries" feature will strip name using `bpaf::cargo_helper` from `cargo shear"
// See <https://docs.rs/bpaf/latest/bpaf/batteries/fn.cargo_helper.html>
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options("shear"))]
pub struct Options {
    #[bpaf(positional("PATH"), fallback(PathBuf::from(".")))]
    path: PathBuf,
}

pub fn shear(options: &Options) {
    let metadata = MetadataCommand::new().current_dir(&options.path).exec().unwrap();
    let workspace_root = metadata.workspace_root.as_std_path();

    for package in metadata.workspace_packages() {
        shear_package(workspace_root, package);
    }

    shear_workspace(&metadata);
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
        .flat_map(|p| &p.dependencies)
        .map(|p| p.name.clone())
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
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
                    .map(|e| e.into_path())
                    .collect::<Vec<_>>()
            }
        })
        .collect::<HashSet<_>>();

    let rust_file_deps = rust_file_paths
        .iter()
        .filter_map(|path| process_rust_source(path))
        .reduce(|a, b| a.union(&b).cloned().collect())
        .unwrap_or_default();

    let package_deps =
        package.dependencies.iter().map(|d| d.name.replace('-', "_")).collect::<HashSet<_>>();

    let unused_deps = package_deps.difference(&rust_file_deps).collect::<Vec<_>>();

    if !unused_deps.is_empty() {
        println!("{:?}: {unused_deps:?}", dir.strip_prefix(workspace_root).unwrap());
    }
}

fn process_rust_source(path: &Path) -> Option<Deps> {
    let source_text = fs::read_to_string(path).unwrap();
    let Ok(syntax) = syn::parse_str::<syn::File>(&source_text) else { return None };
    let mut collector = ImportsCollector::default();
    collector.visit(&syntax);
    Some(collector.deps)
}

#[derive(Default)]
struct ImportsCollector {
    deps: Deps,
}

impl ImportsCollector {
    fn visit(&mut self, syntax: &syn::File) {
        use syn::visit::Visit;
        self.visit_file(syntax);
    }

    fn is_known_import(s: &str) -> bool {
        matches!(s, "crate" | "super" | "self" | "std")
    }
}

impl<'a> syn::visit::Visit<'a> for ImportsCollector {
    /// A path prefix of imports in a use item: `std::....`
    fn visit_use_path(&mut self, use_path: &'a syn::UsePath) {
        let ident = use_path.ident.to_string();
        if Self::is_known_import(&ident) {
            return;
        }
        self.deps.insert(ident);
        syn::visit::visit_use_path(self, use_path);
    }

    /// A path at which a named item is exported (e.g. `std::collections::HashMap`).
    ///
    /// This also gets crate level or renamed imports (I don't know how to fix yet).
    fn visit_path(&mut self, path: &'a syn::Path) {
        if path.segments.len() <= 1 {
            return;
        }
        let Some(path_segment) = path.segments.first() else { return };
        let ident = path_segment.ident.to_string();
        if Self::is_known_import(&ident) || ident.chars().next().is_some_and(|c| c.is_uppercase()) {
            return;
        }
        self.deps.insert(ident);
        syn::visit::visit_path(self, path);
    }
}
