use std::{collections::BTreeMap, fmt, ops::Range};

use rustc_hash::FxHashSet;
use serde::Deserialize;
use toml::Spanned;

#[derive(Debug, Clone)]
pub struct FeatureDep {
    pub name: String,
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepTable {
    Normal,
    Dev,
    Build,
}

impl fmt::Display for DepTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => f.write_str("dependencies"),
            Self::Dev => f.write_str("dev-dependencies"),
            Self::Build => f.write_str("build-dependencies"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepLocation {
    /// Top level table
    Root(DepTable),
    /// Target specific table
    Target { cfg: String, table: DepTable },
}

impl DepLocation {
    #[must_use]
    pub fn as_table(&self, new: DepTable) -> Self {
        match self {
            Self::Root(_) => Self::Root(new),
            Self::Target { cfg, .. } => Self::Target { cfg: cfg.clone(), table: new },
        }
    }
}

impl fmt::Display for DepLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(table) => write!(f, "[{table}]"),
            Self::Target { cfg, table } => write!(f, "[target.'{cfg}'.{table}]"),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct CargoShearMetadata {
    #[serde(default)]
    pub ignored: FxHashSet<Spanned<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PackageMetadata {
    #[serde(default, rename = "cargo-shear")]
    pub cargo_shear: CargoShearMetadata,
}

#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceMetadata {
    #[serde(default, rename = "cargo-shear")]
    pub cargo_shear: CargoShearMetadata,
}

#[derive(Debug, Deserialize, Default)]
pub struct Package {
    #[serde(default)]
    pub metadata: PackageMetadata,
}

#[derive(Debug, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub workspace: Workspace,
    #[serde(default)]
    pub package: Package,
    #[serde(default)]
    pub dependencies: DepsSet,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: DepsSet,
    #[serde(default, rename = "build-dependencies")]
    pub build_dependencies: DepsSet,
    #[serde(default)]
    pub target: BTreeMap<String, Target>,
    #[serde(default)]
    pub features: BTreeMap<Spanned<String>, Vec<Spanned<String>>>,
}

impl Manifest {
    // Iterate over all dependencies, with included location
    pub fn all_dependencies(
        &self,
    ) -> impl Iterator<Item = (&Spanned<String>, &Spanned<Dependency>, DepLocation)> {
        let dependencies = self
            .dependencies
            .iter()
            .map(|(key, value)| (key, value, DepLocation::Root(DepTable::Normal)));

        let dev_dependencies = self
            .dev_dependencies
            .iter()
            .map(|(key, value)| (key, value, DepLocation::Root(DepTable::Dev)));

        let build_dependencies = self
            .build_dependencies
            .iter()
            .map(|(key, value)| (key, value, DepLocation::Root(DepTable::Build)));

        let target_dependencies = self.target.iter().flat_map(|(cfg, target)| {
            let location = |table| DepLocation::Target { cfg: cfg.clone(), table };

            let target_dependencies = target
                .dependencies
                .iter()
                .map(move |(key, value)| (key, value, location(DepTable::Normal)));

            let target_dev_dependencies = target
                .dev_dependencies
                .iter()
                .map(move |(key, value)| (key, value, location(DepTable::Dev)));

            let target_build_dependencies = target
                .build_dependencies
                .iter()
                .map(move |(key, value)| (key, value, location(DepTable::Build)));

            target_dependencies.chain(target_dev_dependencies).chain(target_build_dependencies)
        });

        dependencies.chain(dev_dependencies).chain(build_dependencies).chain(target_dependencies)
    }
}

pub type DepsSet = BTreeMap<Spanned<String>, Spanned<Dependency>>;

#[derive(Debug, Deserialize, Default)]
pub struct Workspace {
    #[serde(default)]
    pub metadata: WorkspaceMetadata,
    #[serde(default)]
    pub dependencies: DepsSet,
}

#[derive(Debug, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub dependencies: DepsSet,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: DepsSet,
    #[serde(default, rename = "build-dependencies")]
    pub build_dependencies: DepsSet,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    #[expect(unused, reason = "TODO")]
    Simple(String),
    Detailed(DependencyDetail),
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct DependencyDetail {
    pub package: Option<String>,
    #[serde(default)]
    pub optional: bool,
}

impl Dependency {
    #[must_use]
    pub fn package(&self) -> Option<&str> {
        match self {
            Self::Simple(_) => None,
            Self::Detailed(detail) => detail.package.as_deref(),
        }
    }

    #[must_use]
    pub const fn optional(&self) -> bool {
        match self {
            Self::Simple(_) => false,
            Self::Detailed(detail) => detail.optional,
        }
    }
}
