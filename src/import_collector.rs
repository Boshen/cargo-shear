//! Import statement collector for cargo-shear.
//!
//! This module parses Rust source code using `syn` to extract all import
//! statements and references to external crates. It handles various forms
//! of imports including:
//!
//! - `use` statements
//! - `extern crate` declarations
//! - Path references in code (e.g., `std::collections::HashMap`)
//! - Macro invocations
//! - Attribute references (e.g., `#[derive(...)]`)

use std::sync::OnceLock;

use regex_lite::Regex;
use syn::{self, ext::IdentExt, spanned::Spanned};

use crate::dependency_analyzer::Dependencies as Deps;

/// Collect all import statements and crate references from Rust source code.
///
/// This function parses the source text and extracts all references to external
/// crates, whether they come from use statements, macro invocations, or inline paths.
///
/// # Arguments
///
/// * `source_text` - The Rust source code to analyze
///
/// # Returns
///
/// A set of crate names that are referenced in the source code
pub fn collect_imports(source_text: &str) -> syn::Result<Deps> {
    let syntax = syn::parse_str::<syn::File>(source_text)?;
    let mut collector = ImportCollector::default();
    collector.visit(&syntax);
    Ok(collector.deps)
}

#[derive(Default)]
struct ImportCollector {
    deps: Deps,
}

impl ImportCollector {
    fn visit(&mut self, syntax: &syn::File) {
        use syn::visit::Visit;
        self.visit_file(syntax);
    }

    fn is_known_import(s: &str) -> bool {
        matches!(s, "crate" | "super" | "self" | "std")
    }

    fn add_import(&mut self, s: String) {
        if !Self::is_known_import(&s) {
            self.deps.insert(s);
        }
    }

    fn unraw_string(ident: &syn::Ident) -> String {
        ident.unraw().to_string()
    }

    fn add_ident(&mut self, ident: &syn::Ident) {
        self.add_import(Self::unraw_string(ident));
    }

    fn collect_use_tree(&mut self, i: &syn::UseTree) {
        use syn::UseTree;
        match i {
            UseTree::Path(use_path) => self.add_ident(&use_path.ident),
            UseTree::Name(use_name) => self.add_ident(&use_name.ident),
            UseTree::Rename(use_rename) => self.add_ident(&use_rename.ident),
            UseTree::Glob(_) => {}
            UseTree::Group(use_group) => {
                for use_tree in &use_group.items {
                    self.collect_use_tree(use_tree);
                }
            }
        }
    }

    // `foo::bar` in expressions
    fn collect_path(&mut self, path: &syn::Path, is_module: bool) {
        if path.segments.len() <= 1 && !is_module {
            // Avoid collecting single-segment paths unless they explicitly point to a module, which might be a crate.
            // This prevents false positives from free functions and other local items.
            return;
        }
        let Some(path_segment) = path.segments.first() else { return };
        let ident = Self::unraw_string(&path_segment.ident);
        if ident.chars().next().is_some_and(char::is_uppercase) {
            return;
        }
        self.add_import(ident);
    }

    // `let _: <foo::bar>`
    fn collect_type_path(&mut self, type_path: &syn::TypePath) {
        let path = &type_path.path;
        self.collect_path(path, false);
    }

    // `println!("{}", foo::bar);`
    //                 ^^^^^^^^ search for the `::` pattern
    fn collect_tokens(&mut self, tokens: &proc_macro2::TokenStream) {
        static MACRO_RE: OnceLock<Regex> = OnceLock::new();
        let Some(source_text) = tokens.span().source_text() else { return };
        let idents = MACRO_RE
            .get_or_init(|| {
                Regex::new(r"(?:r#)?([A-Za-z_]\w*)[\s]*::[\s]*(?:r#)?([A-Za-z_]\w*)")
                    .unwrap_or_else(|e| panic!("Failed to parse regex {e:?}"))
            })
            .captures_iter(&source_text)
            .filter_map(|c| c.get(1))
            .map(|m| m.as_str())
            .filter(|s| !Self::is_known_import(s))
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        self.deps.extend(idents);
    }

    // #[serde(with = "foo")]
    fn collect_known_attribute(&mut self, attr: &syn::Attribute) {
        // Many serde attributes are already caught by `collect_tokens` because they use the `::` pattern.
        // However, the `with` and `crate` attributes are special cases since they directly reference modules or crates.
        if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|meta| {
                // #[serde(with = "foo")]
                // #[serde(crate = "foo")]
                if meta.path.is_ident("with") || meta.path.is_ident("crate") {
                    let _eq = meta.input.parse::<syn::Token![=]>()?;
                    let lit = meta.input.parse::<syn::LitStr>()?;
                    let path = syn::parse_str(&lit.value())?;
                    self.collect_path(&path, true);
                }
                // ignore unknown args
                Ok(())
            })
            // Ignore invalid serde attributes.
            .ok();
        }
    }
}

impl<'a> syn::visit::Visit<'a> for ImportCollector {
    fn visit_path(&mut self, i: &'a syn::Path) {
        self.collect_path(i, false);
        syn::visit::visit_path(self, i);
    }

    /// A use declaration: `use std::collections::HashMap`.
    fn visit_item_use(&mut self, i: &'a syn::ItemUse) {
        self.collect_use_tree(&i.tree);
    }

    /// A path like `std::slice::Iter`, optionally qualified with a self-type as in <Vec<T> as `SomeTrait>::Associated`.
    fn visit_type_path(&mut self, i: &'a syn::TypePath) {
        self.collect_type_path(i);
        syn::visit::visit_type_path(self, i);
    }

    /// A structured list within an attribute, like derive(Copy, Clone).
    fn visit_meta_list(&mut self, m: &'a syn::MetaList) {
        self.collect_path(&m.path, false);
        self.collect_tokens(&m.tokens);
    }

    /// An extern crate item: extern crate serde.
    fn visit_item_extern_crate(&mut self, i: &'a syn::ItemExternCrate) {
        self.add_ident(&i.ident);
    }

    fn visit_macro(&mut self, m: &'a syn::Macro) {
        self.collect_path(&m.path, false);
        self.collect_tokens(&m.tokens);
    }

    fn visit_item(&mut self, i: &'a syn::Item) {
        // For tokens not interpreted by Syn.
        if let syn::Item::Verbatim(tokens) = i {
            self.collect_tokens(tokens);
        }
        syn::visit::visit_item(self, i);
    }

    fn visit_attribute(&mut self, attr: &'a syn::Attribute) {
        self.collect_known_attribute(attr);
        syn::visit::visit_attribute(self, attr);
    }
}
