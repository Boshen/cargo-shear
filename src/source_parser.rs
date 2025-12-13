//! Parses Rust source files to extract imports and file paths.
//! Uses `ra_ap_syntax` for parsing Rust and `pulldown-cmark` for doc comments.

#![expect(
    clippy::wildcard_enum_match_arm,
    reason = "Hundreds of SyntaxKind variants, matching on '_' is the best option"
)]

use std::{
    io,
    path::{Path as StdPath, PathBuf},
};

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use ra_ap_syntax::{
    AstNode, AstToken, Edition, NodeOrToken, SourceFile, SyntaxKind, SyntaxNode, SyntaxToken,
    ast::{
        Attr, Comment, CommentShape, ExternCrate, HasAttrs, HasModuleItem, HasName, MacroCall,
        MacroRules, Module, Path, String as AstString, TokenTree, Use, UseTree,
    },
};
use rustc_hash::FxHashSet;

use crate::util::read_to_string;

/// Result of parsing a source file.
#[derive(Debug, Default)]
pub struct ParsedSource {
    /// Imports referenced in the source.
    pub imports: FxHashSet<String>,

    /// Rust paths referenced by this source file.
    pub paths: FxHashSet<PathBuf>,

    /// Whether this file is empty (no items, only whitespace/comments).
    pub is_empty: bool,
}

impl ParsedSource {
    /// Parse a file.
    pub fn from_path(path: &StdPath, is_entry_point: bool) -> io::Result<Self> {
        let source = read_to_string(path)?;
        Ok(SourceParser::parse(&source, Some(path), is_entry_point, false))
    }

    /// Parse source code.
    pub fn from_str(source: &str, path: &StdPath) -> Self {
        SourceParser::parse(source, Some(path), true, false)
    }

    /// Parse expanded source code (from `cargo expand`).
    /// Absolute paths are skipped due to macro hygiene.
    pub fn from_expanded_str(source: &str, path: &StdPath) -> Self {
        SourceParser::parse(source, Some(path), true, true)
    }

    /// Parse source code with explicit entry point flag.
    #[cfg(test)]
    pub fn from_path_test(source: &str, path: &StdPath, is_entry_point: bool) -> Self {
        SourceParser::parse(source, Some(path), is_entry_point, false)
    }
}

struct SourceParser {
    result: ParsedSource,

    /// Parent directory.
    /// Used for resolving `include!` declarations.
    parent: Option<PathBuf>,

    /// Module directory.
    /// Used for resolving `mod` declarations.
    module: Option<PathBuf>,

    // TODO: Consider trying to parse comments as we walk the source?
    /// All doc comments, merged into one markdown string.
    markdown: String,

    /// Whether we're parsing expanded code from `cargo expand`.
    /// When true, absolute paths (e.g., `::tracing::debug!`) are skipped
    /// because they resolve through macro-defining crates due to macro hygiene.
    is_expanded: bool,
}

impl SourceParser {
    fn new(path: Option<&StdPath>, is_entry_point: bool, is_expanded: bool) -> Self {
        let parent = path.and_then(|path| path.parent()).map(StdPath::to_path_buf);

        // Entry points: modules resolve relative to parent directory.
        // Regular files: modules resolve relative to sibling directory.
        let module = path.and_then(|path| {
            let parent = path.parent()?;

            if is_entry_point {
                return Some(parent.to_path_buf());
            }

            match path.file_stem().and_then(|stem| stem.to_str()) {
                Some("mod") | None => Some(parent.to_path_buf()),
                Some(stem) => Some(parent.join(stem)),
            }
        });

        Self {
            result: ParsedSource::default(),
            parent,
            module,
            markdown: String::new(),
            is_expanded,
        }
    }

    fn parse(
        source: &str,
        path: Option<&StdPath>,
        is_entry_point: bool,
        is_expanded: bool,
    ) -> ParsedSource {
        let mut parser = Self::new(path, is_entry_point, is_expanded);

        let tree = SourceFile::parse(source, Edition::CURRENT);
        let tree = tree.tree();

        // Check if file is empty (no items, only whitespace/comments)
        parser.result.is_empty = tree.items().next().is_none();

        for element in tree.syntax().descendants_with_tokens() {
            parser.visit(element);
        }

        parser.parse_markdown();
        parser.result
    }

    fn parse_markdown(&mut self) {
        if self.markdown.is_empty() {
            return;
        }

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
            SyntaxKind::MODULE => self.visit_module(node),
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
        let Some(macro_call) = MacroCall::cast(node.clone()) else { return };

        if let Some(path) = macro_call.path() {
            self.collect_path(&path);
        }

        if let Some(token_tree) = macro_call.token_tree() {
            self.collect_token_tree(&token_tree);
        }

        self.collect_macro_call(&macro_call);
    }

    fn visit_macro_rules(&mut self, node: &SyntaxNode) {
        if let Some(token_tree) =
            MacroRules::cast(node.clone()).and_then(|macro_rules| macro_rules.token_tree())
        {
            self.collect_token_tree(&token_tree);
        }
    }

    fn visit_attribute(&mut self, node: &SyntaxNode) {
        let Some(attr) = Attr::cast(node.clone()) else { return };

        if let Some(token_tree) = attr.token_tree() {
            self.collect_token_tree(&token_tree);

            // Special casing for known attributes
            if attr.path().is_some_and(|path| path.to_string() == "serde") {
                self.collect_serde_attribute(&token_tree);
            }
        }
    }

    fn visit_module(&mut self, node: &SyntaxNode) {
        // Nested modules will already be handled by `collect_module`
        if node
            .ancestors()
            .any(|ancestor| ancestor.kind() == SyntaxKind::MODULE && ancestor != *node)
        {
            return;
        }

        let Some(module) = Module::cast(node.clone()) else { return };
        let module_dir = self.module.clone();
        let path_dir = self.parent.clone();
        self.collect_module(&module, module_dir.as_deref(), path_dir.as_deref());
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

    /// Collect from a token tree.
    fn collect_token_tree(&mut self, token_tree: &TokenTree) {
        let tokens: Vec<_> = token_tree
            .syntax()
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| !token.kind().is_trivia())
            .collect();

        // Re-parse inner content as Rust.
        let needs_reparse = tokens.iter().any(|token| {
            matches!(token.kind(), SyntaxKind::USE_KW | SyntaxKind::EXTERN_KW | SyntaxKind::MOD_KW)
        });

        if needs_reparse {
            let text = token_tree.syntax().text().to_string();
            let text = text
                .strip_prefix(['{', '(', '['])
                .and_then(|text| text.strip_suffix(['}', ')', ']']))
                .unwrap_or(&text);

            if !text.is_empty() {
                let parsed = SourceFile::parse(text, Edition::CURRENT);
                for element in parsed.tree().syntax().descendants_with_tokens() {
                    self.visit(element);
                }
            }
        }

        self.collect_path_imports(&tokens);
    }

    /// Collect imports from a raw token stream
    fn collect_path_imports(&mut self, tokens: &[SyntaxToken]) {
        for (index, token) in tokens.iter().enumerate() {
            // Look for `::` to find path expressions
            if token.kind() == SyntaxKind::COLON2
                || (token.kind() == SyntaxKind::COLON
                    && tokens.get(index + 1).is_some_and(|next| next.kind() == SyntaxKind::COLON))
            {
                self.collect_path_import(tokens, index);
            }
        }
    }

    /// Collect path import
    /// - `foo::bar::baz`
    /// - `::foo::bar::baz`
    fn collect_path_import(&mut self, tokens: &[SyntaxToken], index: usize) {
        let prev = tokens.get(index.wrapping_sub(1));

        if let Some(prev) = prev.filter(|token| token.kind() == SyntaxKind::IDENT) {
            // Relative path: `foo::bar`
            let before_prev = tokens.get(index.wrapping_sub(2));

            let is_continuation = before_prev.is_some_and(|token| {
                token.kind() == SyntaxKind::COLON2
                    || (token.kind() == SyntaxKind::COLON
                        && tokens
                            .get(index.wrapping_sub(3))
                            .is_some_and(|token| token.kind() == SyntaxKind::COLON))
            });

            if !is_continuation {
                self.add_import(prev.text());
            }
        } else if !self.is_expanded {
            // Absolute path: `::foo::bar`
            // Skip absolute paths in expanded code due to macro hygiene:
            // they resolve through the macro-defining crate, not the current crate.
            let next_index =
                if tokens[index].kind() == SyntaxKind::COLON2 { index + 1 } else { index + 2 };

            if let Some(next) =
                tokens.get(next_index).filter(|token| token.kind() == SyntaxKind::IDENT)
            {
                self.add_import(next.text());
            }
        }
    }

    /// Collect imports from a `use` tree.
    /// - `use foo::bar` -> `foo`
    /// - `use foo::bar::baz` -> `foo`
    /// - `use {foo, bar}` -> `foo`, `bar`
    /// - `use foo::{bar, baz}` -> `foo`
    /// - `use ::foo::bar` -> skipped in expanded code (absolute path)
    fn collect_use_tree(&mut self, tree: &UseTree) {
        if let Some(path) = tree.path() {
            // Skip absolute paths in expanded code due to macro hygiene
            if self.is_expanded && path.to_string().starts_with("::") {
                return;
            }

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
    /// - `::foo::bar()` -> skipped in expanded code (absolute path)
    fn collect_path(&mut self, path: &Path) {
        // Skip absolute paths in expanded code due to macro hygiene
        if self.is_expanded && path.to_string().starts_with("::") {
            return;
        }

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

    /// Collect from macro calls:
    /// - `include!("foo.rs")` -> `foo.rs`
    /// - `include_proto!("../proto.rs")` -> `../proto.rs`
    fn collect_macro_call(&mut self, macro_call: &MacroCall) {
        let Some(parent) = &self.parent else { return };
        let Some(token_tree) = macro_call.token_tree() else { return };

        for token in token_tree
            .syntax()
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| token.kind() == SyntaxKind::STRING)
            .filter_map(AstString::cast)
        {
            let Ok(value) = token.value() else { continue };
            if !value.ends_with(".rs") {
                continue;
            }

            self.result.paths.insert(parent.join(value.as_ref()));
        }
    }

    /// Collect from module:
    /// - `mod foo;` -> `foo.rs`, `foo/mod.rs`
    /// - `mod foo { mod bar; }` -> `foo/bar.rs`, `foo/bar/mod.rs`
    fn collect_module(
        &mut self,
        module: &Module,
        module_dir: Option<&StdPath>,
        path_dir: Option<&StdPath>,
    ) {
        let Some(ident) = module.name() else { return };
        let text = ident.text();
        let name = text.strip_prefix("r#").unwrap_or_else(|| text.as_ref());

        let paths: Vec<_> =
            module.attrs().flat_map(|attr| Self::extract_path_attr(&attr)).collect();

        // Inline module:
        // `mod foo { ... }`
        if let Some(item_list) = module.item_list() {
            self.add_module_path(name, module_dir);

            // Add explicit #[path = "..."] paths
            for path in &paths {
                if StdPath::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
                {
                    self.add_explicit_path(path, path_dir);
                }
            }

            // Recurse into child modules:
            // `mod foo { mod bar; }` -> `foo/bar.rs`
            // `#[path = "x"] mod foo { mod bar; }` -> `x/bar.rs`
            let subdir = paths.first().map_or(name, String::as_str);
            let subdir = subdir.strip_suffix(".rs").unwrap_or(subdir);

            let next_module_dir = module_dir.unwrap_or_else(|| StdPath::new("")).join(subdir);
            let next_path_dir = path_dir.unwrap_or_else(|| StdPath::new("")).join(subdir);

            for item in item_list.items() {
                if let Some(child) = Module::cast(item.syntax().clone()) {
                    self.collect_module(&child, Some(&next_module_dir), Some(&next_path_dir));
                }
            }

            return;
        }

        // External module:
        // `mod foo;`

        // Add explicit #[path = "..."] paths
        for path in &paths {
            self.add_explicit_path(path, path_dir);
        }

        // Add default paths unless there's an unconditional #[path = "..."]
        // (cfg_attr paths are conditional, so we still need defaults for those)
        let has_path_attr = module.attrs().any(|attr| {
            attr.path().is_some_and(|path| path.to_string() == "path") && attr.expr().is_some()
        });

        if !has_path_attr {
            self.add_module_path(name, module_dir);
        }
    }

    /// Extract `path = "..."`
    fn extract_path_attr(attr: &Attr) -> Vec<String> {
        let mut paths = vec![];

        let mut tokens = attr
            .syntax()
            .descendants_with_tokens()
            .filter_map(NodeOrToken::into_token)
            .filter(|token| !token.kind().is_trivia())
            .peekable();

        while let Some(token) = tokens.next() {
            if token.kind() == SyntaxKind::IDENT
                && token.text() == "path"
                && tokens.next_if(|next| next.kind() == SyntaxKind::EQ).is_some()
                && let Some(value) = tokens.next_if(|next| next.kind() == SyntaxKind::STRING)
                && let Some(string) = AstString::cast(value)
                && let Ok(value) = string.value()
            {
                paths.push(value.to_string());
            }
        }

        paths
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

    /// Add module paths: `foo.rs` and `foo/mod.rs`
    fn add_module_path(&mut self, name: &str, dir: Option<&StdPath>) {
        let Some(module) = &self.module else { return };

        let name = name.strip_prefix("r#").unwrap_or(name);
        let base = dir.map_or_else(|| module.clone(), |dir| module.join(dir));

        self.result.paths.insert(base.join(format!("{name}.rs")));
        self.result.paths.insert(base.join(format!("{name}/mod.rs")));
    }

    /// Add explicit path from `#[path = "..."]`
    fn add_explicit_path(&mut self, path: &str, dir: Option<&StdPath>) {
        let Some(parent) = &self.parent else { return };

        let base = dir.unwrap_or(parent);
        self.result.paths.insert(base.join(path));
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(parsed.imports, FxHashSet::from_iter(["url".to_owned()]));
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

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(parsed.imports, FxHashSet::from_iter(["async_trait".to_owned()]));
    }

    #[test]
    fn collects_imports_from_statement_based_doctest() {
        let source = r#"
        /// ```rust
        /// let value = serde_json::json!({"key": "value"});
        /// let serialized = serde_json::to_string(&value).unwrap();
        /// ```
        fn example() {}
        "#;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(parsed.imports, FxHashSet::from_iter(["serde_json".to_owned()]));
    }

    #[test]
    fn collects_paths_modules() {
        let source = r"
            mod foo;
            pub mod bar;
            mod r#box;

            mod inline {
                mod child;
            }

            mod a { mod b { mod c { mod d; } } }
        ";

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // foo
                PathBuf::from("foo.rs"),
                PathBuf::from("foo/mod.rs"),
                // bar
                PathBuf::from("bar.rs"),
                PathBuf::from("bar/mod.rs"),
                // box
                PathBuf::from("box.rs"),
                PathBuf::from("box/mod.rs"),
                // inline
                PathBuf::from("inline.rs"),
                PathBuf::from("inline/mod.rs"),
                PathBuf::from("inline/child.rs"),
                PathBuf::from("inline/child/mod.rs"),
                // a/b/c/d
                PathBuf::from("a.rs"),
                PathBuf::from("a/mod.rs"),
                PathBuf::from("a/b.rs"),
                PathBuf::from("a/b/mod.rs"),
                PathBuf::from("a/b/c.rs"),
                PathBuf::from("a/b/c/mod.rs"),
                PathBuf::from("a/b/c/d.rs"),
                PathBuf::from("a/b/c/d/mod.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_path() {
        let source = r#"
            #[path = "custom/path.rs"]
            mod foo;

            #[path = "../sibling/mod.rs"]
            mod sibling;

            #[path = "implementations"]
            mod impls {
                mod bar;
            }
        "#;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // foo
                PathBuf::from("custom/path.rs"),
                // sibling
                PathBuf::from("../sibling/mod.rs"),
                // impls
                PathBuf::from("impls.rs"),
                PathBuf::from("impls/mod.rs"),
                PathBuf::from("implementations/bar.rs"),
                PathBuf::from("implementations/bar/mod.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_cfg() {
        let source = r#"
            #[cfg_attr(feature = "foo", path = "foo_impl.rs")]
            mod impl_module;

            #[cfg_attr(feature = "v1", path = "v1.rs")]
            #[cfg_attr(feature = "v2", path = "v2.rs")]
            mod versioned;

            #[cfg(feature = "serde")]
            mod serde_support;

            #[cfg(test)]
            mod tests {
                mod fixtures;
            }
        "#;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // impl_module
                PathBuf::from("foo_impl.rs"),
                PathBuf::from("impl_module.rs"),
                PathBuf::from("impl_module/mod.rs"),
                // versioned
                PathBuf::from("v1.rs"),
                PathBuf::from("v2.rs"),
                PathBuf::from("versioned.rs"),
                PathBuf::from("versioned/mod.rs"),
                // serde_support
                PathBuf::from("serde_support.rs"),
                PathBuf::from("serde_support/mod.rs"),
                // tests
                PathBuf::from("tests.rs"),
                PathBuf::from("tests/mod.rs"),
                PathBuf::from("tests/fixtures.rs"),
                PathBuf::from("tests/fixtures/mod.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_macros() {
        let source = r##"
            include!("generated/code.rs");
            include!(r#"path/to/file.rs"#);

            const SOURCE: &str = include_str!("./minicore.rs");
            const DATA: &[u8] = include_bytes!("data.rs");

            fake_macro!("some/path.rs", "other_arg");
            foo::bar::baz!("nested/call.rs");
        "##;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // include
                PathBuf::from("generated/code.rs"),
                PathBuf::from("path/to/file.rs"),
                // include_str
                PathBuf::from("./minicore.rs"),
                // include_bytes
                PathBuf::from("data.rs"),
                // fake_macro
                PathBuf::from("some/path.rs"),
                // foo::bar::baz
                PathBuf::from("nested/call.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_macro() {
        let source = r#"
            m! {
                mod foo;
                mod bar;
            }

            outer! {
                inner! {
                    mod nested;
                }
            }

            macro_rules! root {
                () => {
                    pub mod de;
                };
            }

            n! {
                #[path = "custom.rs"]
                mod explicit;
            }

            o! {
                mod inline {
                    mod child;
                }
            }
        "#;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // m
                PathBuf::from("foo.rs"),
                PathBuf::from("foo/mod.rs"),
                PathBuf::from("bar.rs"),
                PathBuf::from("bar/mod.rs"),
                // outer
                PathBuf::from("nested.rs"),
                PathBuf::from("nested/mod.rs"),
                // macro_rules
                PathBuf::from("de.rs"),
                PathBuf::from("de/mod.rs"),
                // n - #[path] attribute provides explicit path, no default paths
                PathBuf::from("custom.rs"),
                // o - inline module with nested child
                PathBuf::from("inline.rs"),
                PathBuf::from("inline/mod.rs"),
                PathBuf::from("inline/child.rs"),
                PathBuf::from("inline/child/mod.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_expanded() {
        let source = r#"
            mod normalize {
                #[path = "tests.rs"]
                mod tests {
                    #[path = "and-n-others.rs"]
                    mod and_n_others {
                        fn test() {}
                    }

                    #[path = "basic.rs"]
                    mod basic {
                        fn test() {}
                    }
                }
            }
        "#;

        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                // normalize
                PathBuf::from("normalize.rs"),
                PathBuf::from("normalize/mod.rs"),
                // normalize/tests
                PathBuf::from("normalize/tests.rs"),
                PathBuf::from("normalize/tests/mod.rs"),
                // and_n_others
                PathBuf::from("normalize/tests/and_n_others.rs"),
                PathBuf::from("normalize/tests/and_n_others/mod.rs"),
                PathBuf::from("normalize/tests/and-n-others.rs"),
                // basic
                PathBuf::from("normalize/tests/basic.rs"),
                PathBuf::from("normalize/tests/basic/mod.rs"),
            ])
        );
    }

    #[test]
    fn collects_paths_nested_attr() {
        // Example from the `tokio` repo.
        let parsed = ParsedSource::from_path_test(
            r#"
            #[cfg(windows)]
            #[path = "windows/sys.rs"]
            mod imp;

            #[cfg(not(windows))]
            #[path = "windows/stub.rs"]
            mod imp;
            "#,
            Path::new("signal/windows.rs"),
            false,
        );
        assert_eq!(
            parsed.paths,
            FxHashSet::from_iter([
                PathBuf::from("signal/windows/sys.rs"),
                PathBuf::from("signal/windows/stub.rs"),
            ])
        );
    }

    #[test]
    fn skips_absolute_paths_in_expanded_code() {
        // Simulates expanded macro code with absolute paths.
        // These absolute paths resolve through macro-defining crates due to macro hygiene,
        // not through the current crate's dependencies.
        let source = r#"
            fn main() {
                {
                    use ::tracing::__macro_support::Callsite as _;
                    ::tracing::debug!("test");
                }
                
                // Relative paths should still be collected
                foo::bar();
            }
        "#;

        // Parse as expanded code
        let parsed = ParsedSource::from_expanded_str(source, Path::new("lib.rs"));

        // Should NOT collect 'tracing' (absolute path ::tracing)
        // Should collect 'foo' (relative path foo::bar)
        assert_eq!(parsed.imports, FxHashSet::from_iter(["foo".to_owned()]));
    }

    #[test]
    fn collects_absolute_paths_in_normal_code() {
        // In normal (non-expanded) code, absolute paths should be collected
        let source = r#"
            fn main() {
                ::tracing::debug!("test");
                foo::bar();
            }
        "#;

        // Parse as normal code
        let parsed = ParsedSource::from_str(source, Path::new("lib.rs"));

        // Should collect both 'tracing' and 'foo'
        assert_eq!(parsed.imports, FxHashSet::from_iter(["tracing".to_owned(), "foo".to_owned()]));
    }
}
