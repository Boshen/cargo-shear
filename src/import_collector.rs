
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
    collect_imports_internal(source_text, true)
}

fn collect_imports_internal(source_text: &str, include_doc_code: bool) -> syn::Result<Deps> {
    let syntax = syn::parse_str::<syn::File>(source_text)?;
    let mut deps = collect_from_syntax(&syntax, include_doc_code);

    if include_doc_code {
        for block in gather_doc_blocks(source_text) {
            let normalized = normalize_doc_block(&block);
            if normalized.trim().is_empty() {
                continue;
            }
            if let Some(snippet_deps) = collect_imports_from_snippet(&normalized) {
                deps.extend(snippet_deps);
            }
        }
    }

    Ok(deps)
}

fn collect_from_syntax(syntax: &syn::File, include_doc_code: bool) -> Deps {
    let mut collector = ImportCollector::new(include_doc_code);
    collector.visit(syntax);
    collector.deps
}

fn collect_imports_from_snippet(code: &str) -> Option<Deps> {
    // Try parsing as a complete file first
    if let Ok(syntax) = syn::parse_file(code) {
        return Some(collect_from_syntax(&syntax, false));
    }

     // If that fails, wrap in a main function (like doc tests do)
     let wrapped = format!("fn main() {{\n{code}\n}}");
     let syntax = syn::parse_file(&wrapped).ok()?;
    Some(collect_from_syntax(&syntax, false))
}

struct ImportCollector {
    deps: Deps,
    include_doc_code: bool,
}

impl ImportCollector {
    fn new(include_doc_code: bool) -> Self {
        Self { deps: Deps::default(), include_doc_code }
    }

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
        let Some(source_text) = tokens.span().source_text() else { return };

        let idents = source_text
            .match_indices("::")
            .filter_map(|(pos, _)| Self::extract_identifier_before(&source_text, pos))
            .filter(|ident| !Self::is_known_import(ident));

        self.deps.extend(idents);
    }

    // Helper function to extract identifier before a given position
    fn extract_identifier_before(text: &str, pos: usize) -> Option<String> {
        let bytes = text.as_bytes();
        let mut end = pos;

        // Skip any whitespace before ::
        while end > 0 && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        }

        if end == 0 {
            return None;
        }

        // Check for raw identifier (r#)
        let is_raw = end >= 2 && &bytes[end - 2..end] == b"r#";
        let ident_end = if is_raw { end - 2 } else { end };

        // Find the start of the identifier
        let mut start = ident_end;
        while start > 0 {
            let prev = start - 1;
            let ch = bytes[prev];
            if ch.is_ascii_alphabetic() || ch == b'_' || (start < ident_end && ch.is_ascii_digit())
            {
                start = prev;
            } else {
                break;
            }
        }

        // If we're looking at a raw identifier, adjust the start
        if is_raw && start >= 2 && &bytes[start - 2..start] == b"r#" {
            start -= 2;
        }

        // Check if there's another :: before this identifier (i.e., this is part of a longer path)
        // We only want to capture the first segment of paths like foo::bar::baz
        if start >= 2 && bytes[start - 2] == b':' && bytes[start - 1] == b':' {
            return None;
        }

        if start < ident_end {
            let full_ident = &text[start..end];
            // Remove r# prefix if present for the actual identifier
            let ident =
                full_ident.strip_prefix("r#").map_or_else(|| full_ident.to_owned(), str::to_owned);

            // Validate it's a proper identifier
            if ident.chars().next()?.is_ascii_alphabetic() || ident.starts_with('_') {
                return Some(ident);
            }
        }

        None
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
        if !self.include_doc_code && attr.path().is_ident("doc") {
            return;
        }
        self.collect_known_attribute(attr);
        syn::visit::visit_attribute(self, attr);
    }
}

fn gather_doc_blocks(source_text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current_doc = Vec::new();

    for line in source_text.lines() {
        if let Some(content) = extract_line_doc(line) {
            current_doc.push(content.to_owned());
        } else if !current_doc.is_empty() {
            let doc_text = current_doc.join("\n");
            blocks.extend(extract_fenced_code_blocks(&doc_text));
            current_doc.clear();
        }
    }

    if !current_doc.is_empty() {
        let doc_text = current_doc.join("\n");
        blocks.extend(extract_fenced_code_blocks(&doc_text));
    }

    let mut search_start = 0;
    while search_start < source_text.len() {
        let slice = &source_text[search_start..];
        let Some((relative_start, marker_len)) = find_next_block_doc(slice) else {
            break;
        };
        let absolute_start = search_start + relative_start;
        let content_start = absolute_start + marker_len;
        let remainder = &source_text[content_start..];
        let Some(end) = remainder.find("*/") else {
            break;
        };
        let raw_block = &remainder[..end];
        let doc_text = extract_block_doc_text(raw_block);
        blocks.extend(extract_fenced_code_blocks(&doc_text));
        search_start = content_start + end + 2;
    }

    blocks
}

 fn extract_line_doc(line: &str) -> Option<&str> {
     let trimmed = line.trim_start();
     trimmed.strip_prefix("///").map_or_else(|| trimmed.strip_prefix("//!"), Some)
 }

fn find_next_block_doc(slice: &str) -> Option<(usize, usize)> {
    let star = slice.find("/**");
    let bang = slice.find("/*!");
    match (star, bang) {
        (Some(a), Some(b)) => {
            if a <= b {
                Some((a, 3))
            } else {
                Some((b, 3))
            }
        }
        (Some(a), None) => Some((a, 3)),
        (None, Some(b)) => Some((b, 3)),
        (None, None) => None,
    }
}

fn extract_block_doc_text(raw: &str) -> String {
    raw.lines()
        .map(|line| {
            let trimmed = line.trim_start();
            let without_star = trimmed.strip_prefix('*').map_or(trimmed, |rest| {
                rest.strip_prefix(' ').or_else(|| rest.strip_prefix('\t')).unwrap_or(rest)
            });
            without_star.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_fenced_code_blocks(doc_text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = doc_text.lines().collect();
    let mut idx = 0;

    while idx < lines.len() {
        let line = lines[idx];
        if let Some(info) = line.trim_start().strip_prefix("```") {
            let include = should_include_info(info.trim());
            idx += 1;
            let mut snippet_lines = Vec::new();
            while idx < lines.len() && !lines[idx].trim_start().starts_with("```") {
                snippet_lines.push(lines[idx].to_owned());
                idx += 1;
            }
            if include {
                blocks.push(snippet_lines.join("\n"));
            }
            if idx < lines.len() {
                idx += 1;
            }
        } else {
            idx += 1;
        }
    }

    blocks
}

fn should_include_info(info: &str) -> bool {
    if info.is_empty() {
        return true;
    }
    let lower = info.to_ascii_lowercase();
    lower
        .split(|c: char| c.is_ascii_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .any(|part| matches!(part, "rust" | "ignore" | "no_run" | "should_panic"))
}

/// Doc-test snippets hide setup lines with `#`; strip those markers and normalize indentation before parsing.
fn normalize_doc_block(code: &str) -> String {
    let mut lines: Vec<String> = code.lines().map(strip_hidden_prefix).collect();

    let indent = lines
        .iter()
        .filter_map(
            |line| {
                if line.trim().is_empty() { None } else { Some(leading_whitespace(line)) }
            },
        )
        .min()
        .unwrap_or(0);

    for line in &mut lines {
        if line.trim().is_empty() {
            line.clear();
        } else if indent > 0 {
            *line = trim_leading_whitespace(line, indent);
        }
    }

    lines.join("\n")
}

fn strip_hidden_prefix(line: &str) -> String {
    let leading = line.bytes().take_while(|b| *b == b' ' || *b == b'\t').count();
    let rest = &line[leading..];
    rest.strip_prefix('#').map_or_else(|| line.to_owned(), |stripped| {
        let stripped =
            stripped.strip_prefix(' ').or_else(|| stripped.strip_prefix('\t')).unwrap_or(stripped);
        let prefix = &line[..leading];
        format!("{prefix}{stripped}")
    })
}

fn leading_whitespace(line: &str) -> usize {
    line.bytes().take_while(|b| *b == b' ' || *b == b'\t').count()
}

fn trim_leading_whitespace(line: &str, count: usize) -> String {
    if count == 0 {
        return line.to_owned();
    }
    let mut idx = 0;
    let mut removed = 0;
    let bytes = line.as_bytes();
    while idx < bytes.len() && removed < count {
        let b = bytes[idx];
        if b == b' ' || b == b'\t' {
            idx += 1;
            removed += 1;
        } else {
            break;
        }
    }
    line[idx..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_imports_from_doc_rust_block() {
        let source = r#"
        /// Parses URLs.
        ///
        /// ```rust
        /// # use url::Url;
        /// let url = Url::parse("https://example.com").unwrap();
        /// println!("{}", url);
        /// ```
        fn demo() {}
        "#;

        let deps = collect_imports(source).expect("failed to collect imports from doc block");
        assert!(deps.contains("url"), "doc-test rust blocks should count as dependency usage");
    }
}
