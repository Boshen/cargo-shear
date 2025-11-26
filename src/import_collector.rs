//! Import statement collector for cargo-shear.
//!
//! This module parses Rust source code using `ra_ap_syntax` to extract all import
//! statements and references to external crates. It handles various forms
//! of imports including:
//!
//! - `use` statements
//! - `extern crate` declarations
//! - Path references in code (e.g., `std::collections::HashMap`)
//! - Macro invocations
//! - Attribute references (e.g., `#[derive(...)]`)

#![expect(
    clippy::wildcard_enum_match_arm,
    reason = "Hundreds of SyntaxKind variants, matching on '_' is the best option"
)]

use ra_ap_syntax::{
    AstNode, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken, WalkEvent,
    ast::{Attr, ExternCrate, MacroCall, MacroRules, Path, TokenTree, Use, UseTree},
};
use rustc_hash::FxHashSet;

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
pub fn collect_imports(source_text: &str) -> FxHashSet<String> {
    collect_imports_internal(source_text, true)
}

fn collect_imports_internal(source_text: &str, include_doc_code: bool) -> FxHashSet<String> {
    let syntax = SourceFile::parse(source_text, Edition::CURRENT);
    let mut deps = collect_from_syntax(&syntax.tree(), include_doc_code);

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

    deps
}

fn collect_from_syntax(syntax: &SourceFile, include_doc_code: bool) -> FxHashSet<String> {
    let mut collector = ImportCollector::new(include_doc_code);
    collector.visit(syntax);
    collector.deps
}

fn collect_imports_from_snippet(code: &str) -> Option<FxHashSet<String>> {
    // Try parsing as a complete file first
    let syntax = SourceFile::parse(code, Edition::CURRENT);
    if syntax.errors().is_empty() {
        return Some(collect_from_syntax(&syntax.tree(), false));
    }

    // If that fails, wrap in a main function (like doc tests do)
    let wrapped = format!("fn main() {{\n{code}\n}}");
    let syntax = SourceFile::parse(&wrapped, Edition::CURRENT);
    if syntax.errors().is_empty() {
        return Some(collect_from_syntax(&syntax.tree(), false));
    }

    None
}

struct ImportCollector {
    deps: FxHashSet<String>,
    include_doc_code: bool,
}

impl ImportCollector {
    fn new(include_doc_code: bool) -> Self {
        Self { deps: FxHashSet::default(), include_doc_code }
    }

    fn visit(&mut self, syntax: &SourceFile) {
        for event in syntax.syntax().preorder() {
            let WalkEvent::Enter(node) = event else { continue };

            match node.kind() {
                SyntaxKind::USE => self.visit_use(node),
                SyntaxKind::EXTERN_CRATE => self.visit_extern_crate(node),
                SyntaxKind::PATH => self.visit_path(node),
                SyntaxKind::MACRO_CALL => self.visit_macro_call(node),
                SyntaxKind::MACRO_RULES => self.visit_macro_rules(node),
                SyntaxKind::ATTR => self.visit_attribute(node),
                _ => {}
            }
        }
    }

    fn is_known_import(s: &str) -> bool {
        matches!(s, "crate" | "super" | "self" | "std")
    }

    fn add_import(&mut self, s: &str) {
        if !Self::is_known_import(s) {
            // Handle raw identifiers
            let clean = s.strip_prefix("r#").unwrap_or(s);
            self.deps.insert(clean.to_owned());
        }
    }

    fn collect_use_tree(&mut self, tree: &UseTree) {
        // Path imports
        // - `use foo::bar`
        // - `use foo::{bar, baz}`
        // - `use foo as bar`
        if let Some(path) = tree.path() {
            // Extract the first segment
            if let Some(first_segment) = path.segments().next()
                && let Some(name_ref) = first_segment.name_ref()
            {
                self.add_import(name_ref.text().as_ref());
            }
        }

        // Group imports
        // - `use {foo, bar}`
        // - `use foo::{bar, baz}`
        if let Some(use_tree_list) = tree.use_tree_list()
            && tree.path().is_none()
        {
            for subtree in use_tree_list.use_trees() {
                self.collect_use_tree(&subtree);
            }
        }
    }

    // `foo::bar` in expressions
    fn collect_path(&mut self, path: &Path, is_module: bool) {
        if path.segments().count() <= 1 && !is_module {
            // Avoid collecting single-segment paths unless they explicitly point to a module, which might be a crate.
            // This prevents false positives from free functions and other local items.
            return;
        }
        let Some(path_segment) = path.segments().next() else { return };
        let Some(name_ref) = path_segment.name_ref() else { return };
        let ident = name_ref.text();
        if ident.chars().next().is_some_and(char::is_uppercase) {
            return;
        }
        self.add_import(ident.as_ref());
    }

    // `println!("{}", foo::bar);`
    //                 ^^^^^^^^ search for the `::` pattern
    fn collect_tokens(&mut self, node: &SyntaxNode) {
        let tokens: Vec<SyntaxToken> = node
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|t| !t.kind().is_trivia())
            .collect();

        for (i, token) in tokens.iter().enumerate() {
            if i == 0 {
                continue;
            }

            let is_path_sep = match token.kind() {
                SyntaxKind::COLON2 => true,
                SyntaxKind::COLON => {
                    i + 1 < tokens.len() && tokens[i + 1].kind() == SyntaxKind::COLON
                }
                _ => false,
            };

            if is_path_sep {
                // Check that prev token is NOT also preceded by ::
                // (we only want the first segment of a path)
                let preceded_by_path_sep = i >= 2 && {
                    let before_prev = &tokens[i - 2];
                    before_prev.kind() == SyntaxKind::COLON2
                        || (before_prev.kind() == SyntaxKind::COLON
                            && i >= 3
                            && tokens[i - 3].kind() == SyntaxKind::COLON)
                };

                if !preceded_by_path_sep {
                    let prev = &tokens[i - 1];
                    if prev.kind() == SyntaxKind::IDENT && Self::is_valid_import_ident(prev.text())
                    {
                        self.add_import(prev.text());
                    }
                }
            }
        }
    }

    fn is_valid_import_ident(text: &str) -> bool {
        if text.is_empty() {
            return false;
        }

        // Skip known keywords
        if Self::is_known_import(text) {
            return false;
        }

        // Handle raw identifiers
        let clean = text.strip_prefix("r#").unwrap_or(text);

        // Must start with letter or underscore
        clean.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    }

    // #[serde(with = "foo")]
    fn collect_serde_attribute(&mut self, token_tree: &TokenTree) {
        // Many serde attributes are already caught by `collect_tokens` because they use the `::` pattern.
        // However, the `with` and `crate` attributes are special cases since they directly reference modules or crates.
        let text = token_tree.syntax().text().to_string();

        // #[serde(with = "foo")]
        // #[serde(crate = "foo")]
        for pattern in ["with = \"", "crate = \""] {
            let Some(rest) = text.split_once(pattern).map(|(_, after)| after) else {
                continue;
            };

            let Some((path, _)) = rest.split_once('"') else { continue };

            // Extract first segment
            if let Some(import) = path.split("::").next() {
                self.add_import(import);
            }
        }
    }

    fn visit_path(&mut self, node: SyntaxNode) {
        // Skip paths inside use statements, they're handled by collect_use_tree
        if node.ancestors().any(|n| n.kind() == SyntaxKind::USE) {
            return;
        }
        let Some(path) = Path::cast(node) else { return };
        self.collect_path(&path, false);
    }

    /// A use declaration: `use std::collections::HashMap`
    fn visit_use(&mut self, node: SyntaxNode) {
        let Some(use_item) = Use::cast(node) else { return };
        let Some(use_tree) = use_item.use_tree() else { return };
        self.collect_use_tree(&use_tree);
    }

    /// An extern crate item: `extern crate serde`
    fn visit_extern_crate(&mut self, node: SyntaxNode) {
        let Some(extern_crate) = ExternCrate::cast(node) else { return };
        let Some(name_ref) = extern_crate.name_ref() else { return };
        self.add_import(name_ref.text().as_ref());
    }

    /// A macro invocation: `println!("hello")`
    fn visit_macro_call(&mut self, node: SyntaxNode) {
        let Some(macro_call) = MacroCall::cast(node) else { return };

        if let Some(path) = macro_call.path() {
            self.collect_path(&path, false);
        }

        if let Some(token_tree) = macro_call.token_tree() {
            self.collect_tokens(token_tree.syntax());
        }
    }

    /// A `macro_rules` definition: `macro_rules! foo { ... }`
    fn visit_macro_rules(&mut self, node: SyntaxNode) {
        let Some(macro_rules) = MacroRules::cast(node) else { return };
        let Some(token_tree) = macro_rules.token_tree() else { return };
        self.collect_tokens(token_tree.syntax());
    }

    /// An attribute: `#[derive(Debug)]`
    fn visit_attribute(&mut self, node: SyntaxNode) {
        let Some(attr) = Attr::cast(node) else { return };

        if !self.include_doc_code
            && let Some(path) = attr.path()
            && path.to_string() == "doc"
        {
            return;
        }

        if let Some(token_tree) = attr.token_tree() {
            self.collect_tokens(token_tree.syntax());

            // Handle known attributes
            if let Some(path) = attr.path()
                && path.to_string() == "serde"
            {
                self.collect_serde_attribute(&token_tree);
            }
        }
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

    // Only strip '#' if it's followed by a space/tab
    match rest.as_bytes() {
        [b'#', b' ' | b'\t', ..] => {
            let prefix = &line[..leading];
            let stripped = &rest[2..];
            format!("{prefix}{stripped}")
        }
        _ => line.to_owned(),
    }
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

        let deps = collect_imports(source);
        assert!(deps.contains("url"), "doc-test rust blocks should count as dependency usage");
    }

    #[test]
    fn collects_imports_from_doc_block_with_attribute() {
        let source = r"
        /// ```rust
        /// # use async_trait::async_trait;
        /// #[async_trait]
        /// trait HttpClient {
        ///     async fn send(request: Request);
        /// }
        /// ```
        fn example() {}
        ";

        let deps = collect_imports(source);
        assert!(deps.contains("async_trait"), "should detect async_trait");
    }

    #[test]
    fn collects_imports_from_statement_based_doctest() {
        // Regression test: doctest snippets often contain bare statements (let bindings, expressions)
        // that aren't valid at module scope. The parser should wrap them in `fn main() {}`.
        let source = r#"
        /// Demonstrates usage with statement-based doctest.
        ///
        /// ```rust
        /// let value = serde_json::json!({"key": "value"});
        /// let serialized = serde_json::to_string(&value).unwrap();
        /// assert!(!serialized.is_empty());
        /// ```
        fn example() {}
        "#;

        let deps = collect_imports(source);
        assert!(
            deps.contains("serde_json"),
            "statement-based doctests should be wrapped and parsed correctly"
        );
    }
}
