use std::{collections::BTreeMap, fmt};

use cargo_metadata::TargetKind;
use globset::{Glob, GlobMatcher};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Deserializer};
use toml::Spanned;

/// How a dependency is referenced in `[features]`.
///
/// See: <https://doc.rust-lang.org/cargo/reference/features.html>
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FeatureRef {
    /// Implicit feature from an optional dependency.
    Implicit,

    /// Explicit dependency reference: `dep:foo` or `foo`.
    Explicit { feature: Spanned<String>, value: Spanned<String> },

    /// Dependency feature enablement: `foo/bar`.
    DepFeature { feature: Spanned<String>, value: Spanned<String> },

    /// Weak dependency feature enablement: `foo?/bar`.
    WeakDepFeature { feature: Spanned<String>, value: Spanned<String> },
}

impl FeatureRef {
    pub fn parse(feature: &Spanned<String>, value: &Spanned<String>) -> (String, Self) {
        if let Some(dep) = value.as_ref().strip_prefix("dep:") {
            let import = dep.replace('-', "_");
            return (import, Self::Explicit { feature: feature.clone(), value: value.clone() });
        }

        if let Some((dep, _)) = value.as_ref().split_once('/') {
            let is_weak = dep.ends_with('?');
            let dep = dep.trim_end_matches('?');
            let import = dep.replace('-', "_");

            let feature = if is_weak {
                Self::WeakDepFeature { feature: feature.clone(), value: value.clone() }
            } else {
                Self::DepFeature { feature: feature.clone(), value: value.clone() }
            };

            return (import, feature);
        }

        let import = value.as_ref().replace('-', "_");
        (import, Self::Explicit { feature: feature.clone(), value: value.clone() })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepTable {
    Normal,
    Dev,
    Build,
}

impl From<&TargetKind> for DepTable {
    fn from(kind: &TargetKind) -> Self {
        match kind {
            TargetKind::CustomBuild => Self::Build,
            TargetKind::Test | TargetKind::Bench | TargetKind::Example => Self::Dev,
            TargetKind::Bin
            | TargetKind::CDyLib
            | TargetKind::DyLib
            | TargetKind::Lib
            | TargetKind::ProcMacro
            | TargetKind::RLib
            | TargetKind::StaticLib
            | TargetKind::Unknown(_)
            | _ => Self::Normal,
        }
    }
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
    pub const fn is_normal(&self) -> bool {
        matches!(self, Self::Root(DepTable::Normal) | Self::Target { table: DepTable::Normal, .. })
    }

    /// Move this location to a different table.
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

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum Dependency {
    #[expect(unused, reason = "Needed for deserialization")]
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

pub type DepsSet = BTreeMap<Spanned<String>, Spanned<Dependency>>;

#[derive(Debug, Deserialize)]
pub struct Target {
    #[serde(default)]
    pub dependencies: DepsSet,
    #[serde(default, rename = "dev-dependencies")]
    pub dev_dependencies: DepsSet,
    #[serde(default, rename = "build-dependencies")]
    pub build_dependencies: DepsSet,
}

#[derive(Deserialize, Default)]
pub struct ShearConfig {
    #[serde(default)]
    pub ignored: FxHashSet<Spanned<String>>,
    #[serde(default, rename = "ignored-paths")]
    pub ignored_paths: Vec<SpannedGlob>,
}

#[derive(Deserialize, Default)]
pub struct Metadata {
    #[serde(default, rename = "cargo-shear")]
    pub cargo_shear: ShearConfig,
}

#[derive(Deserialize, Default)]
pub struct Package {
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Deserialize, Default)]
pub struct Workspace {
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default)]
    pub dependencies: DepsSet,
}

#[derive(Deserialize)]
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
    /// Iterate over all dependencies, with included location.
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

#[derive(Debug, Clone)]
pub struct SpannedGlob {
    pub pattern: Spanned<String>,
    pub matcher: GlobMatcher,
}

impl<'de> Deserialize<'de> for SpannedGlob {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let pattern = Spanned::<String>::deserialize(deserializer)?;
        let glob = Glob::new(pattern.get_ref()).map_err(serde::de::Error::custom)?;
        Ok(Self { pattern, matcher: glob.compile_matcher() })
    }
}
