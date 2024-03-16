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

    // `use foo::bar;`
    fn collect_use_path(&mut self, use_path: &syn::UsePath) {
        let ident = use_path.ident.to_string();
        self.add_import(ident);
    }

    // `foo::bar` in expressions
    fn collect_path(&mut self, path: &syn::Path) {
        if path.segments.len() <= 1 {
            return;
        }
        let Some(path_segment) = path.segments.first() else { return };
        let ident = path_segment.ident.to_string();
        if ident.chars().next().is_some_and(|c| c.is_uppercase()) {
            return;
        }
        self.add_import(ident);
    }

    // `let _: <foo:bar>`
    fn collect_type_path(&mut self, type_path: &syn::TypePath) {
        let path = &type_path.path;
        self.collect_path(path);
    }

    // `println!("{}", foo::bar);`
    //                 ^^^^^^^^ search for the `::` pattern
    fn collect_macro(&mut self, m: &syn::Macro) {
        self.collect_path(&m.path);
        let Some(source_text) = m.tokens.span().source_text() else { return };
        static MACRO_RE: OnceLock<Regex> = OnceLock::new();
        let idents = MACRO_RE
            .get_or_init(|| Regex::new(r"(\w+)::(\w+)").unwrap())
            .captures_iter(&source_text)
            .filter_map(|c| c.get(1))
            .map(|m| m.as_str())
            .filter(|s| !Self::is_known_import(s))
            .map(ToString::to_string);
        self.deps.extend(idents);
    }
}

impl<'a> syn::visit::Visit<'a> for ImportCollector {
    /// A path prefix of imports in a use item: `std::....`
    fn visit_use_path(&mut self, i: &'a syn::UsePath) {
        self.collect_use_path(i);
        // collect top level use, no need to descend into the use tree
        // syn::visit::visit_use_path(self, i);
    }

    /// A path at which a named item is exported (e.g. `std::collections::HashMap`).
    ///
    /// This also gets crate level or renamed imports (I don't know how to fix yet).
    fn visit_path(&mut self, i: &'a syn::Path) {
        self.collect_path(i);
        syn::visit::visit_path(self, i);
    }

    /// A path like std::slice::Iter, optionally qualified with a self-type as in <Vec<T> as SomeTrait>::Associated.
    fn visit_type_path(&mut self, i: &'a syn::TypePath) {
        self.collect_type_path(i);
        syn::visit::visit_type_path(self, i);
    }

    fn visit_macro(&mut self, i: &'a syn::Macro) {
        self.collect_macro(i);
    }
}

#[cfg(test)]
mod tests {
    use super::collect_imports;
    use std::collections::HashSet;

    #[test]
    fn export_path() {
        let source = r"
          #[test]
          fn box_serialize() {
            let b = foo::bar(&b).unwrap();
          }
        ";
        let deps = collect_imports(source).unwrap();
        let expected = HashSet::from_iter(["foo".to_string()]);
        assert_eq!(deps, expected);
    }

    #[test]
    fn type_path() {
        let source = r"
          fn main() {
            let x: Vec<foo::Bar> = vec![];
          }
        ";
        let deps = collect_imports(source).unwrap();
        let expected = HashSet::from_iter(["foo".to_string()]);
        assert_eq!(deps, expected);
    }

    #[test]
    fn macros() {
        let source = r#"
          fn main() {
            println!("{}", foo::bar);
          }
        "#;
        let deps = collect_imports(source).unwrap();
        let expected = HashSet::from_iter(["foo".to_string()]);
        assert_eq!(deps, expected);
    }
}
