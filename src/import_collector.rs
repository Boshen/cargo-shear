use std::sync::OnceLock;

use regex::Regex;
use syn::{self, spanned::Spanned};

use crate::Deps;

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

    fn add_ident(&mut self, ident: &syn::Ident) {
        self.add_import(ident.to_string());
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
    fn collect_path(&mut self, path: &syn::Path) {
        if path.segments.len() <= 1 {
            return;
        }
        let Some(path_segment) = path.segments.first() else { return };
        let ident = path_segment.ident.to_string();
        if ident.chars().next().is_some_and(char::is_uppercase) {
            return;
        }
        self.add_import(ident);
    }

    // `let _: <foo::bar>`
    fn collect_type_path(&mut self, type_path: &syn::TypePath) {
        let path = &type_path.path;
        self.collect_path(path);
    }

    // `println!("{}", foo::bar);`
    //                 ^^^^^^^^ search for the `::` pattern
    fn collect_tokens(&mut self, tokens: &proc_macro2::TokenStream) {
        static MACRO_RE: OnceLock<Regex> = OnceLock::new();
        let Some(source_text) = tokens.span().source_text() else { return };
        let idents = MACRO_RE
            .get_or_init(|| {
                Regex::new(r"(\w+)::(\w+)")
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
}

impl<'a> syn::visit::Visit<'a> for ImportCollector {
    fn visit_path(&mut self, i: &'a syn::Path) {
        self.collect_path(i);
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
        self.collect_path(&m.path);
        self.collect_tokens(&m.tokens);
    }

    /// An extern crate item: extern crate serde.
    fn visit_item_extern_crate(&mut self, i: &'a syn::ItemExternCrate) {
        self.add_ident(&i.ident);
    }

    fn visit_macro(&mut self, m: &'a syn::Macro) {
        self.collect_path(&m.path);
        self.collect_tokens(&m.tokens);
    }

    fn visit_item(&mut self, i: &'a syn::Item) {
        // For tokens not interpreted by Syn.
        if let syn::Item::Verbatim(tokens) = i {
            self.collect_tokens(tokens);
        }
        syn::visit::visit_item(self, i);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::collect_imports;

    fn test(source_text: &str) {
        let deps = collect_imports(source_text).unwrap();
        let expected = HashSet::from_iter(["foo".to_string()]);
        assert_eq!(deps, expected, "{source_text}");
    }

    #[test]
    fn export_path() {
        test("#[test] fn box_serialize() { let b = foo::bar(&b).unwrap(); }");
    }

    #[test]
    fn type_path() {
        test("fn main() { let x: Vec<foo::Bar> = vec![]; }");
    }

    #[test]
    fn r#macro() {
        test(r#"fn main() { println!("{}", foo::bar); }"#);
    }

    #[test]
    fn use_group() {
        test("pub use { foo };");
    }

    #[test]
    fn meta_list() {
        test("#[derive(foo::Deserialize, foo::Serialize)] struct Foo;");
    }

    #[test]
    fn extern_crate() {
        test("extern crate foo;");
    }

    #[test]
    fn meta_list_path() {
        test(r#"#[foo::instrument(level = "debug")] fn print_with_indent() {}"#);
    }

    #[test]
    fn use_rename() {
        test("use foo as bar;");
    }

    #[test]
    fn macro_on_struct() {
        test("#[foo::self_referencing] struct AST {}");
    }

    #[test]
    fn macro_on_verbatim() {
        test("#[foo::ext(name = ParserExt)] pub impl Parser {}");
    }
}
