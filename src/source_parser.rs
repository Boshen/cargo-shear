//! Parses Rust source files to extract imports.
//! Uses `ra_ap_syntax` for parsing Rust and `pulldown-cmark` for doc comments.

#![expect(
    clippy::wildcard_enum_match_arm,
    reason = "Hundreds of SyntaxKind variants, matching on '_' is the best option"
)]

use std::{io, path::Path as StdPath};

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use ra_ap_syntax::{
    AstNode, AstToken, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{
        Attr, Comment, CommentShape, ExternCrate, MacroCall, MacroRules, Path, String as AstString,
        TokenTree, Use, UseTree,
    },
};
use rustc_hash::FxHashSet;

/// Result of parsing a source file.
#[derive(Debug, Default)]
pub struct ParsedSource {
    /// Imports referenced in the source.
    pub imports: FxHashSet<String>,
}

impl ParsedSource {
    /// Parse a file.
    pub fn from_path(path: &StdPath) -> io::Result<Self> {
        let source = std::fs::read_to_string(path)?;
        Ok(SourceParser::parse(&source))
    }

    /// Parse source code.
    pub fn from_str(source: &str) -> Self {
        SourceParser::parse(source)
    }
}

struct SourceParser {
    result: ParsedSource,

    // TODO: Consider trying to parse comments as we walk the source?
    /// All doc comments, merged into one markdown string.
    markdown: String,
}

impl SourceParser {
    fn new() -> Self {
        Self { result: ParsedSource::default(), markdown: String::new() }
    }

    fn parse(source: &str) -> ParsedSource {
        let mut parser = Self::new();

        let tree = SourceFile::parse(source, Edition::CURRENT);
        let tree = tree.tree();

        for element in tree.syntax().descendants_with_tokens() {
            parser.visit(element);
        }

        parser.parse_markdown();
        parser.result
    }

    fn parse_markdown(&mut self) {
        let mut current: Option<String> = None;

        let markdown = std::mem::take(&mut self.markdown);
        for event in Parser::new(&markdown) {
            match event {
                Event::Start(Tag::CodeBlock(kind)) => {
                    let is_rust = match kind {
                        CodeBlockKind::Indented => true,
                        CodeBlockKind::Fenced(info) => {
                            // Empty fence defaults to Rust
                            if info.is_empty() {
                                true
                            } else {
                                // Check for Rust related tags
                                info.split(',').any(|tag| {
                                    matches!(
                                        tag.trim(),
                                        "rust" | "ignore" | "no_run" | "should_panic"
                                    )
                                })
                            }
                        }
                    };

                    if is_rust {
                        current = Some(String::new());
                    }
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(code) = current.take()
                        && !code.trim().is_empty()
                    {
                        let snippet = SourceFile::parse(&code, Edition::CURRENT);
                        for node in snippet.tree().syntax().descendants() {
                            self.visit_node(&node);
                        }
                    }
                }
                Event::Text(text) => {
                    if let Some(code) = &mut current {
                        for line in text.lines() {
                            if !code.is_empty() {
                                code.push('\n');
                            }

                            // Strip hidden line prefix
                            let stripped =
                                line.strip_prefix('#').map_or(line, |rest| rest.trim_start());

                            code.push_str(stripped);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn visit(&mut self, element: NodeOrToken<SyntaxNode, SyntaxToken>) {
        match element {
            NodeOrToken::Node(node) => self.visit_node(&node),
            NodeOrToken::Token(token) => self.visit_token(token),
        }
    }

    fn visit_node(&mut self, node: &SyntaxNode) {
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

    fn visit_token(&mut self, token: SyntaxToken) {
        if token.kind() == SyntaxKind::COMMENT {
            self.visit_comment(token);
        }
    }

    fn visit_use(&mut self, node: &SyntaxNode) {
        if let Some(use_tree) = Use::cast(node.clone()).and_then(|use_item| use_item.use_tree()) {
            self.collect_use_tree(&use_tree);
        }
    }

    fn visit_extern_crate(&mut self, node: &SyntaxNode) {
        if let Some(name_ref) =
            ExternCrate::cast(node.clone()).and_then(|extern_crate| extern_crate.name_ref())
        {
            self.add_import(name_ref.text().as_ref());
        }
    }

    fn visit_path(&mut self, node: &SyntaxNode) {
        // Paths inside `use` statements will already be handled by `visit_use`
        if node
            .ancestors()
            .find(|node| !matches!(node.kind(), SyntaxKind::PATH | SyntaxKind::PATH_SEGMENT))
            .is_some_and(|node| matches!(node.kind(), SyntaxKind::USE_TREE | SyntaxKind::USE))
        {
            return;
        }

        if let Some(path) = Path::cast(node.clone()) {
            self.collect_path(&path);
        }
    }

    fn visit_macro_call(&mut self, node: &SyntaxNode) {
        if let Some(macro_call) = MacroCall::cast(node.clone()) {
            if let Some(path) = macro_call.path() {
                self.collect_path(&path);
            }

            if let Some(token_tree) = macro_call.token_tree() {
                self.collect_tokens(token_tree.syntax());
            }
        }
    }

    fn visit_macro_rules(&mut self, node: &SyntaxNode) {
        if let Some(token_tree) =
            MacroRules::cast(node.clone()).and_then(|macro_rules| macro_rules.token_tree())
        {
            self.collect_tokens(token_tree.syntax());
        }
    }

    fn visit_attribute(&mut self, node: &SyntaxNode) {
        let Some(attr) = Attr::cast(node.clone()) else { return };

        if let Some(token_tree) = attr.token_tree() {
            self.collect_tokens(token_tree.syntax());

            // Special casing for known attribute
            if attr.path().is_some_and(|path| path.to_string() == "serde") {
                self.collect_serde_attribute(&token_tree);
            }
        }
    }

    fn visit_comment(&mut self, token: SyntaxToken) {
        let Some(comment) = Comment::cast(token) else { return };
        let Some((text, _)) = comment.doc_comment() else { return };

        for line in text.lines() {
            let line = match comment.kind().shape {
                CommentShape::Line => line.trim_start(),
                CommentShape::Block => line.trim_start().trim_start_matches('*').trim_start(),
            };

            self.markdown.push_str(line);
            self.markdown.push('\n');
        }

        self.markdown.push('\n');
    }

    /// Collect imports from a raw token stream
    fn collect_tokens(&mut self, node: &SyntaxNode) {
        let tokens: Vec<_> = node
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| !token.kind().is_trivia())
            .collect();

        for (index, token) in tokens.iter().enumerate() {
            match token.kind() {
                // `extern crate foo;`
                SyntaxKind::CRATE_KW => {
                    if index >= 1
                        && tokens[index - 1].kind() == SyntaxKind::EXTERN_KW
                        && let Some(next) = tokens.get(index + 1)
                        && next.kind() == SyntaxKind::IDENT
                    {
                        self.add_import(next.text());
                    }
                }
                // `use foo;`
                // `use {foo, bar::baz};`
                SyntaxKind::USE_KW => {
                    let Some(next) = tokens.get(index + 1) else { continue };
                    match next.kind() {
                        SyntaxKind::IDENT => self.add_import(next.text()),
                        SyntaxKind::L_CURLY => {
                            // Collect first segment only
                            let mut after_path_sep = false;
                            for token in &tokens[index + 2..] {
                                match token.kind() {
                                    SyntaxKind::COLON2 | SyntaxKind::COLON => after_path_sep = true,
                                    SyntaxKind::COMMA => after_path_sep = false,
                                    SyntaxKind::IDENT if !after_path_sep => {
                                        self.add_import(token.text());
                                    }
                                    SyntaxKind::R_CURLY => break,
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                // `foo::bar`
                SyntaxKind::COLON2 => {
                    self.collect_path_import(&tokens, index);
                }
                SyntaxKind::COLON
                    if tokens
                        .get(index + 1)
                        .is_some_and(|next| next.kind() == SyntaxKind::COLON) =>
                {
                    self.collect_path_import(&tokens, index);
                }
                _ => {}
            }
        }
    }

    /// Collect import from `foo::bar::baz`.
    fn collect_path_import(&mut self, tokens: &[SyntaxToken], index: usize) {
        if index < 1 {
            return;
        }

        // Skip if not the first `::` in the path
        if index >= 2 {
            let before_prev = &tokens[index - 2];
            if before_prev.kind() == SyntaxKind::COLON2 {
                return;
            }

            if before_prev.kind() == SyntaxKind::COLON
                && index >= 3
                && tokens[index - 3].kind() == SyntaxKind::COLON
            {
                return;
            }
        }

        let prev = &tokens[index - 1];
        if prev.kind() != SyntaxKind::IDENT {
            return;
        }

        self.add_import(prev.text());
    }

    /// Collect imports from a `use` tree.
    /// - `use foo::bar` -> `foo`
    /// - `use foo::bar::baz` -> `foo`
    /// - `use {foo, bar}` -> `foo`, `bar`
    /// - `use foo::{bar, baz}` -> `foo`
    fn collect_use_tree(&mut self, tree: &UseTree) {
        if let Some(path) = tree.path() {
            if let Some(name_ref) = path.segments().next().and_then(|segment| segment.name_ref()) {
                self.add_import(name_ref.text().as_ref());
            }

            return;
        }

        // Recurse into sub trees.
        if let Some(use_tree_list) = tree.use_tree_list() {
            for subtree in use_tree_list.use_trees() {
                self.collect_use_tree(&subtree);
            }
        }
    }

    /// Collect imports from a path in code.
    /// - `foo::bar()` -> `foo`
    /// - `foo::Bar::new()` -> `foo`
    fn collect_path(&mut self, path: &Path) {
        let mut segments = path.segments();

        // Single segment paths can't be external crates
        if let Some(first) = segments.next()
            && let Some(name_ref) = first.name_ref()
            && segments.next().is_some()
        {
            self.add_import(name_ref.text().as_ref());
        }
    }

    /// Collect from serde attributes:
    /// - `#[serde(with = "serde_regex")]` -> `serde_regex`
    /// - `#[serde(crate = "rocket::serde")]` -> `rocket`
    fn collect_serde_attribute(&mut self, token_tree: &TokenTree) {
        let tokens: Vec<_> = token_tree
            .syntax()
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| !token.kind().is_trivia())
            .collect();

        for window in tokens.windows(3) {
            let [key, eq, string] = window else { continue };

            // `crate` is a keyword, so need to handle both kinds.
            if key.kind() != SyntaxKind::CRATE_KW
                && (key.kind() != SyntaxKind::IDENT || !Self::is_serde_attribute_key(key.text()))
            {
                continue;
            }

            if eq.kind() != SyntaxKind::EQ {
                continue;
            }

            if let Some(string) = AstString::cast(string.clone())
                && let Ok(string) = string.value()
                && let Some(import) = string.split("::").next()
            {
                self.add_import(import);
            }
        }
    }

    fn is_serde_attribute_key(key: &str) -> bool {
        matches!(key, "with" | "deserialize_with" | "serialize_with" | "crate" | "remote")
    }

    fn add_import(&mut self, import: &str) {
        if import.is_empty() {
            return;
        }

        // Skip reserved paths
        if Self::is_known_import(import) {
            return;
        }

        // Handle raw identifiers: r#async -> async
        let clean = import.strip_prefix("r#").unwrap_or(import);

        // Must start with lowercase letter or underscore
        if !clean.chars().next().is_some_and(|char| char.is_ascii_lowercase() || char == '_') {
            return;
        }

        self.result.imports.insert(clean.to_owned());
    }

    fn is_known_import(import: &str) -> bool {
        matches!(import, "crate" | "super" | "self" | "std")
    }
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

        let parsed = ParsedSource::from_str(source);
        assert!(
            parsed.imports.contains("url"),
            "doc-test rust blocks should count as dependency usage"
        );
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

        let parsed = ParsedSource::from_str(source);
        assert!(parsed.imports.contains("async_trait"), "should detect async_trait");
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

        let parsed = ParsedSource::from_str(source);
        assert!(
            parsed.imports.contains("serde_json"),
            "statement-based doctests should be wrapped and parsed correctly"
        );
    }
}
