//! Cargo registry lookup.

use std::path::PathBuf;

use rustc_hash::FxHashMap;
use serde::Deserialize;

use crate::util::read_to_string;

/// Cargo registry source cache
pub struct Registry {
    /// All registry directories
    directories: Vec<PathBuf>,

    /// Cache of lib names, keyed by "{pkg}-{version}".
    cache: FxHashMap<String, Option<String>>,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        let directories = Self::directories();
        Self { directories, cache: FxHashMap::default() }
    }

    fn directories() -> Vec<PathBuf> {
        let Some(root) = home::cargo_home().ok().map(|home| home.join("registry").join("src"))
        else {
            return vec![];
        };

        std::fs::read_dir(&root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect()
    }

    /// Look up the lib name for a package.
    pub fn lookup(&mut self, pkg: &str, version: &str) -> Option<String> {
        let key = format!("{pkg}-{version}");
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }

        let result = self.directories.iter().find_map(|dir| {
            let path = dir.join(&key).join("Cargo.toml");
            let content = read_to_string(&path).ok()?;
            let manifest: CargoToml = toml::from_str(&content).ok()?;
            manifest.lib?.name
        });

        self.cache.insert(key, result.clone());
        result
    }
}

#[derive(Deserialize)]
struct CargoToml {
    lib: Option<LibSection>,
}

#[derive(Deserialize)]
struct LibSection {
    name: Option<String>,
}
