use std::{collections::BTreeSet, error::Error, fmt, path::PathBuf};

use miette::{Diagnostic, LabeledSpan, NamedSource, Severity, SourceSpan};
use rustc_hash::FxHashSet;

use crate::{
    context::{PackageContext, WorkspaceContext},
    manifest::{DepTable, FeatureRef},
    package_processor::{
        EmptyFile, MisplacedDependency, MisplacedOptionalDependency, PackageAnalysis,
        RedundantIgnore, RedundantIgnorePath, UnknownIgnore, UnlinkedFile, UnusedDependency,
        UnusedFeatureDependency, UnusedOptionalDependency, UnusedWorkspaceDependency,
        WorkspaceAnalysis,
    },
};

/// Result of processing all packages across the workspace.
#[derive(Debug, Default)]
pub struct ShearAnalysis {
    /// All diagnostic findings.
    pub findings: Vec<ShearDiagnostic>,

    /// All package names used across the workspace.
    pub packages: FxHashSet<String>,

    /// Count of errors.
    pub errors: usize,

    /// Count of warnings.
    /// Anything that can't be automatically fixed is considered a warning.
    pub warnings: usize,

    /// Count of fixed issues.
    pub fixed: usize,

    /// Whether to show the `ignored` advice.
    pub show_ignored: bool,

    /// Whether to show the `ignored-paths` advice.
    pub show_ignored_paths: bool,
}

impl ShearAnalysis {
    pub fn add_package_result(
        &mut self,
        ctx: &PackageContext<'_>,
        result: &PackageAnalysis,
        fixed: usize,
    ) {
        let relative_path = ctx
            .manifest_path
            .strip_prefix(&ctx.workspace.root)
            .unwrap_or(&ctx.manifest_path)
            .display()
            .to_string();

        let src = NamedSource::new(relative_path, ctx.manifest_content.clone());
        self.packages.extend(result.used_packages.iter().cloned());
        self.fixed += fixed;

        for finding in &result.unused_dependencies {
            self.insert(ShearDiagnostic::unused_dependency(finding, &src));
        }

        for finding in &result.unused_optional_dependencies {
            self.insert(ShearDiagnostic::unused_optional_dependency(finding, &src));
        }

        for finding in &result.unused_feature_dependencies {
            self.insert(ShearDiagnostic::unused_feature_dependency(finding, &src));
        }

        for finding in &result.misplaced_dependencies {
            self.insert(ShearDiagnostic::misplaced_dependency(finding, &src));
        }

        for finding in &result.misplaced_optional_dependencies {
            self.insert(ShearDiagnostic::misplaced_optional_dependency(finding, &src));
        }

        if !result.unlinked_files.is_empty() {
            self.insert(ShearDiagnostic::unlinked_files(&result.unlinked_files, &ctx.name));
        }

        if !result.empty_files.is_empty() {
            self.insert(ShearDiagnostic::empty_files(&result.empty_files, &ctx.name));
        }

        for finding in &result.unknown_ignores {
            self.insert(ShearDiagnostic::unknown_ignore(finding, &src));
        }

        for finding in &result.redundant_ignores {
            self.insert(ShearDiagnostic::redundant_ignore(finding, &src));
        }

        for finding in &result.redundant_ignore_paths {
            self.insert(ShearDiagnostic::redundant_ignore_path(finding, &src));
        }
    }

    pub fn add_workspace_result(
        &mut self,
        ctx: &WorkspaceContext,
        result: &WorkspaceAnalysis,
        fixed: usize,
    ) {
        let src = NamedSource::new("Cargo.toml", ctx.manifest_content.clone());
        self.fixed += fixed;

        for finding in &result.unused_dependencies {
            self.insert(ShearDiagnostic::unused_workspace_dependency(finding, &src));
        }

        for finding in &result.unknown_ignores {
            self.insert(ShearDiagnostic::unknown_ignore(finding, &src));
        }

        for finding in &result.redundant_ignores {
            self.insert(ShearDiagnostic::redundant_ignore(finding, &src));
        }

        for finding in &result.redundant_ignore_paths {
            self.insert(ShearDiagnostic::redundant_ignore_path(finding, &src));
        }
    }

    fn insert(&mut self, diagnostic: ShearDiagnostic) {
        match &diagnostic.kind {
            DiagnosticKind::UnusedDependency { .. }
            | DiagnosticKind::UnusedWorkspaceDependency { .. }
            | DiagnosticKind::MisplacedDependency { .. } => {
                self.errors += 1;
                self.show_ignored = true;
            }
            DiagnosticKind::UnusedOptionalDependency { .. }
            | DiagnosticKind::UnusedFeatureDependency { .. }
            | DiagnosticKind::MisplacedOptionalDependency { .. } => {
                self.warnings += 1;
                self.show_ignored = true;
            }
            DiagnosticKind::UnlinkedFiles { .. } | DiagnosticKind::EmptyFiles { .. } => {
                self.warnings += 1;
                self.show_ignored_paths = true;
            }
            DiagnosticKind::UnknownIgnore { .. }
            | DiagnosticKind::RedundantIgnore { .. }
            | DiagnosticKind::RedundantIgnorePath { .. } => self.warnings += 1,
        }

        self.findings.push(diagnostic);
    }

    /// Whether to show the `--fix` advice.
    pub const fn show_fix(&self) -> bool {
        self.errors > 0 && self.fixed == 0
    }
}

/// Unified diagnostic type that contains all information needed for display.
pub struct ShearDiagnostic {
    /// The kind of diagnostic.
    pub kind: DiagnosticKind,

    /// Source content.
    pub source: Option<NamedSource<String>>,

    /// Primary span.
    pub span: Option<SourceSpan>,

    /// Any related diagnostics.
    pub related: Vec<Box<dyn Diagnostic + Send + Sync>>,

    /// Optional help text.
    pub help: Option<String>,
}

impl fmt::Debug for ShearDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShearDiagnostic")
            .field("kind", &self.kind)
            .field("source", &self.source)
            .field("span", &self.span)
            .field("related", &format!("[{} related diagnostics]", self.related.len()))
            .field("help", &self.help)
            .finish()
    }
}

impl ShearDiagnostic {
    pub fn unused_dependency(diagnostic: &UnusedDependency, source: &NamedSource<String>) -> Self {
        Self {
            kind: DiagnosticKind::UnusedDependency { name: diagnostic.name.get_ref().clone() },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: Vec::new(),
            help: Some("remove this dependency".to_owned()),
        }
    }

    pub fn unused_workspace_dependency(
        diagnostic: &UnusedWorkspaceDependency,
        source: &NamedSource<String>,
    ) -> Self {
        Self {
            kind: DiagnosticKind::UnusedWorkspaceDependency {
                name: diagnostic.name.get_ref().clone(),
            },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: Vec::new(),
            help: Some("remove this dependency".to_owned()),
        }
    }

    pub fn unused_optional_dependency(
        diagnostic: &UnusedOptionalDependency,
        source: &NamedSource<String>,
    ) -> Self {
        Self {
            kind: DiagnosticKind::UnusedOptionalDependency {
                name: diagnostic.name.get_ref().clone(),
            },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: ShearRelatedDiagnostic::from_features(
                Some("removing an optional dependency may be a breaking change"),
                &diagnostic.features,
                source,
            ),
            help: None,
        }
    }

    pub fn unused_feature_dependency(
        diagnostic: &UnusedFeatureDependency,
        source: &NamedSource<String>,
    ) -> Self {
        Self {
            kind: DiagnosticKind::UnusedFeatureDependency {
                name: diagnostic.name.get_ref().clone(),
            },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: ShearRelatedDiagnostic::from_features(None, &diagnostic.features, source),
            help: None,
        }
    }

    pub fn misplaced_dependency(
        diagnostic: &MisplacedDependency,
        source: &NamedSource<String>,
    ) -> Self {
        let target = diagnostic.location.as_table(DepTable::Dev);
        Self {
            kind: DiagnosticKind::MisplacedDependency { name: diagnostic.name.get_ref().clone() },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: Vec::new(),
            help: Some(format!("move this dependency to `{target}`")),
        }
    }

    pub fn misplaced_optional_dependency(
        diagnostic: &MisplacedOptionalDependency,
        source: &NamedSource<String>,
    ) -> Self {
        let target = diagnostic.location.as_table(DepTable::Dev);
        Self {
            kind: DiagnosticKind::MisplacedOptionalDependency {
                name: diagnostic.name.get_ref().clone(),
            },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: ShearRelatedDiagnostic::from_features(
                Some("removing an optional dependency may be a breaking change"),
                &diagnostic.features,
                source,
            ),
            help: Some(format!("remove the `optional` flag and move to `{target}`")),
        }
    }

    pub fn unlinked_files(diagnostics: &[UnlinkedFile], package: &str) -> Self {
        let paths: BTreeSet<_> = diagnostics.iter().map(|file| file.path.clone()).collect();
        let help = if paths.len() == 1 {
            "delete this file".to_owned()
        } else {
            "delete these files".to_owned()
        };

        Self {
            kind: DiagnosticKind::UnlinkedFiles { package: package.to_owned(), paths },
            source: None,
            span: None,
            related: Vec::new(),
            help: Some(help),
        }
    }

    pub fn empty_files(diagnostics: &[EmptyFile], package: &str) -> Self {
        let paths: BTreeSet<_> = diagnostics.iter().map(|file| file.path.clone()).collect();
        let help = if paths.len() == 1 {
            "delete this file".to_owned()
        } else {
            "delete these files".to_owned()
        };

        Self {
            kind: DiagnosticKind::EmptyFiles { package: package.to_owned(), paths },
            source: None,
            span: None,
            related: Vec::new(),
            help: Some(help),
        }
    }

    pub fn unknown_ignore(diagnostic: &UnknownIgnore, source: &NamedSource<String>) -> Self {
        Self {
            kind: DiagnosticKind::UnknownIgnore { name: diagnostic.name.get_ref().clone() },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: Vec::new(),
            help: Some("remove from ignored list".to_owned()),
        }
    }

    pub fn redundant_ignore(diagnostic: &RedundantIgnore, source: &NamedSource<String>) -> Self {
        Self {
            kind: DiagnosticKind::RedundantIgnore { name: diagnostic.name.get_ref().clone() },
            source: Some(source.clone()),
            span: Some(diagnostic.name.span().into()),
            related: Vec::new(),
            help: Some("remove from ignored list".to_owned()),
        }
    }

    pub fn redundant_ignore_path(
        diagnostic: &RedundantIgnorePath,
        source: &NamedSource<String>,
    ) -> Self {
        Self {
            kind: DiagnosticKind::RedundantIgnorePath {
                pattern: diagnostic.pattern.get_ref().clone(),
            },
            source: Some(source.clone()),
            span: Some(diagnostic.pattern.span().into()),
            related: Vec::new(),
            help: Some("remove from ignored paths list".to_owned()),
        }
    }
}

#[derive(Debug)]
pub enum DiagnosticKind {
    UnusedDependency { name: String },
    UnusedWorkspaceDependency { name: String },
    UnusedOptionalDependency { name: String },
    UnusedFeatureDependency { name: String },
    MisplacedDependency { name: String },
    MisplacedOptionalDependency { name: String },
    UnlinkedFiles { package: String, paths: BTreeSet<PathBuf> },
    EmptyFiles { package: String, paths: BTreeSet<PathBuf> },
    UnknownIgnore { name: String },
    RedundantIgnore { name: String },
    RedundantIgnorePath { pattern: String },
}

impl DiagnosticKind {
    pub fn message(&self) -> String {
        match self {
            Self::UnusedDependency { name } => format!("unused dependency `{name}`"),
            Self::UnusedWorkspaceDependency { name } => {
                format!("unused workspace dependency `{name}`")
            }
            Self::UnusedOptionalDependency { name } => {
                format!("unused optional dependency `{name}`")
            }
            Self::UnusedFeatureDependency { name } => {
                format!("dependency `{name}` only used in features")
            }
            Self::MisplacedDependency { name } => format!("misplaced dependency `{name}`"),
            Self::MisplacedOptionalDependency { name } => {
                format!("misplaced optional dependency `{name}`")
            }
            Self::UnlinkedFiles { package, paths } => {
                let count = paths.len();
                let s = if count == 1 { "" } else { "s" };
                let paths = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                format!("{count} unlinked file{s} in `{package}`\n{paths}")
            }
            Self::EmptyFiles { package, paths } => {
                let count = paths.len();
                let s = if count == 1 { "" } else { "s" };
                let paths = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join("\n");

                format!("{count} empty file{s} in `{package}`\n{paths}")
            }
            Self::UnknownIgnore { name } => format!("unknown ignore `{name}`"),
            Self::RedundantIgnore { name } => format!("redundant ignore `{name}`"),
            Self::RedundantIgnorePath { pattern } => {
                format!("redundant ignored paths pattern `{pattern}`")
            }
        }
    }

    pub const fn label(&self) -> Option<&'static str> {
        match self {
            Self::UnusedWorkspaceDependency { .. } => Some("not used by any workspace member"),
            Self::UnusedDependency { .. }
            | Self::UnusedOptionalDependency { .. }
            | Self::UnusedFeatureDependency { .. } => Some("not used in code"),
            Self::MisplacedDependency { .. } | Self::MisplacedOptionalDependency { .. } => {
                Some("only used in dev targets")
            }
            Self::UnlinkedFiles { .. } | Self::EmptyFiles { .. } => None,
            Self::UnknownIgnore { .. } => Some("not a dependency"),
            Self::RedundantIgnore { .. } => Some("dependency is used"),
            Self::RedundantIgnorePath { .. } => Some("pattern not matched"),
        }
    }

    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnusedDependency { .. } => "shear/unused_dependency",
            Self::UnusedWorkspaceDependency { .. } => "shear/unused_workspace_dependency",
            Self::UnusedOptionalDependency { .. } => "shear/unused_optional_dependency",
            Self::UnusedFeatureDependency { .. } => "shear/unused_feature_dependency",
            Self::MisplacedDependency { .. } => "shear/misplaced_dependency",
            Self::MisplacedOptionalDependency { .. } => "shear/misplaced_optional_dependency",
            Self::UnlinkedFiles { .. } => "shear/unlinked_files",
            Self::EmptyFiles { .. } => "shear/empty_files",
            Self::UnknownIgnore { .. } => "shear/unknown_ignore",
            Self::RedundantIgnore { .. } => "shear/redundant_ignore",
            Self::RedundantIgnorePath { .. } => "shear/redundant_ignore_path",
        }
    }

    pub const fn severity(&self) -> Severity {
        match self {
            Self::UnusedDependency { .. }
            | Self::UnusedWorkspaceDependency { .. }
            | Self::MisplacedDependency { .. } => Severity::Error,
            Self::UnlinkedFiles { .. }
            | Self::EmptyFiles { .. }
            | Self::UnusedOptionalDependency { .. }
            | Self::UnusedFeatureDependency { .. }
            | Self::MisplacedOptionalDependency { .. }
            | Self::UnknownIgnore { .. }
            | Self::RedundantIgnore { .. }
            | Self::RedundantIgnorePath { .. } => Severity::Warning,
        }
    }

    /// Returns `true` if this diagnostic can be automatically fixed with `--fix`.
    pub const fn is_fixable(&self) -> bool {
        matches!(
            self,
            Self::UnusedDependency { .. }
                | Self::UnusedWorkspaceDependency { .. }
                | Self::MisplacedDependency { .. }
        )
    }
}

impl fmt::Display for ShearDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind.message())
    }
}

impl Error for ShearDiagnostic {}

impl Diagnostic for ShearDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(self.kind.code()))
    }

    fn severity(&self) -> Option<Severity> {
        Some(self.kind.severity())
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help.as_ref().map(|help| Box::new(help.as_str()) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.source.as_ref().map(|source| source as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let label = self.kind.label()?;
        let span = self.span?;
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(Some(label.to_owned()), span))))
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        if self.related.is_empty() {
            return None;
        }

        Some(Box::new(self.related.iter().map(|diagnostic| diagnostic.as_ref() as &dyn Diagnostic)))
    }
}

/// A related diagnostic.
#[derive(Debug)]
struct ShearRelatedDiagnostic {
    message: String,
    label: Option<(String, SourceSpan, NamedSource<String>)>,
}

impl ShearRelatedDiagnostic {
    fn from_features(
        message: Option<&str>,
        features: &[FeatureRef],
        source: &NamedSource<String>,
    ) -> Vec<Box<dyn Diagnostic + Send + Sync>> {
        let mut related: Vec<Box<dyn Diagnostic + Send + Sync>> = Vec::new();

        if let Some(message) = message {
            related.push(Self { message: message.to_owned(), label: None }.into());
        }

        for feature in features {
            match feature {
                FeatureRef::Explicit { feature, value }
                | FeatureRef::DepFeature { feature, value }
                | FeatureRef::WeakDepFeature { feature, value } => {
                    let name = feature.get_ref();
                    related.push(
                        Self {
                            message: format!("used in feature `{name}`"),
                            label: Some((
                                "enabled here".to_owned(),
                                value.span().into(),
                                source.clone(),
                            )),
                        }
                        .into(),
                    );
                }
                FeatureRef::Implicit => {}
            }
        }

        related
    }
}

impl fmt::Display for ShearRelatedDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for ShearRelatedDiagnostic {}

impl Diagnostic for ShearRelatedDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.label.as_ref().map(|(_, _, source)| source as &dyn miette::SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.label.as_ref().map(|(label, span, _)| {
            Box::new(std::iter::once(LabeledSpan::new_with_span(Some(label.clone()), *span)))
                as Box<dyn Iterator<Item = LabeledSpan> + '_>
        })
    }
}
