//! Minimal tree formatting util.

use std::{collections::BTreeMap, fmt, path::Path};

/// A tree structure for displaying paths.
pub struct Tree {
    root: String,
    children: BTreeMap<String, Node>,
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Self>,
}

impl Tree {
    pub fn new(root: impl Into<String>) -> Self {
        Self { root: root.into(), children: BTreeMap::new() }
    }

    pub fn with_paths<P: AsRef<Path>>(
        root: impl Into<String>,
        paths: impl IntoIterator<Item = P>,
    ) -> Self {
        let mut tree = Self::new(root);
        for path in paths {
            tree.insert(path.as_ref());
        }

        tree
    }

    pub fn insert(&mut self, path: &Path) {
        let mut node = &mut self.children;
        for part in path {
            node = &mut node.entry(part.to_string_lossy().into_owned()).or_default().children;
        }
    }
}

impl fmt::Display for Tree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.root)?;
        fmt_children(f, &self.children, "")
    }
}

fn fmt_children(
    f: &mut fmt::Formatter<'_>,
    children: &BTreeMap<String, Node>,
    prefix: &str,
) -> fmt::Result {
    let children: Vec<_> = children.iter().collect();
    let last = children.len().saturating_sub(1);

    for (index, (name, child)) in children.into_iter().enumerate() {
        let connector = if index == last { "└── " } else { "├── " };
        writeln!(f, "{prefix}{connector}{name}")?;

        if !child.children.is_empty() {
            let extension = if index == last { "    " } else { "│   " };
            fmt_children(f, &child.children, &format!("{prefix}{extension}"))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let tree = Tree::new("crate");
        insta::assert_snapshot!(tree, @r"
        crate
        ");
    }

    #[test]
    fn complex() {
        let mut tree = Tree::new("crate");
        tree.insert(Path::new("src/main.rs"));
        tree.insert(Path::new("src/lib.rs"));
        tree.insert(Path::new("src/util/helpers.rs"));
        tree.insert(Path::new("tests/integration.rs"));
        insta::assert_snapshot!(tree, @r"
        crate
        ├── src
        │   ├── lib.rs
        │   ├── main.rs
        │   └── util
        │       └── helpers.rs
        └── tests
            └── integration.rs
        ");
    }
}
