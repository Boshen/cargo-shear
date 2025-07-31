use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use syn::{self};

/// Collect file references from Rust source code (mod statements, etc.)
pub fn collect_file_references(source_text: &str, current_file: &Path) -> syn::Result<HashSet<PathBuf>> {
    let syntax = syn::parse_str::<syn::File>(source_text)?;
    let mut collector = FileReferenceCollector::new(current_file);
    collector.visit(&syntax);
    Ok(collector.referenced_files)
}

struct FileReferenceCollector {
    referenced_files: HashSet<PathBuf>,
    current_file: PathBuf,
}

impl FileReferenceCollector {
    fn new(current_file: &Path) -> Self {
        Self {
            referenced_files: HashSet::new(),
            current_file: current_file.to_owned(),
        }
    }

    fn visit(&mut self, syntax: &syn::File) {
        use syn::visit::Visit;
        self.visit_file(syntax);
    }

    fn add_mod_reference(&mut self, mod_name: &str) {
        // Handle Rust module resolution rules
        let current_dir = self.current_file.parent().unwrap_or_else(|| Path::new("."));
        
        // Try both module.rs and module/mod.rs patterns
        let candidates = vec![
            current_dir.join(format!("{}.rs", mod_name)),
            current_dir.join(mod_name).join("mod.rs"),
        ];
        
        for candidate in candidates {
            // Canonicalize the path to handle any symbolic links or relative paths
            if let Ok(canonical_path) = candidate.canonicalize() {
                self.referenced_files.insert(canonical_path);
                break;
            }
        }
    }
}

impl<'a> syn::visit::Visit<'a> for FileReferenceCollector {
    fn visit_item_mod(&mut self, i: &'a syn::ItemMod) {
        // Only collect mod statements that reference external files
        // (those without a body, indicated by semicolon)
        if i.content.is_none() {
            self.add_mod_reference(&i.ident.to_string());
        }
        
        syn::visit::visit_item_mod(self, i);
    }
}