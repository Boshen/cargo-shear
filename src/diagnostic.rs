use std::{
    error::Error,
    fmt,
    io::{self, Write},
};

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, Severity,
    SourceCode,
};

use crate::manifest::DepLocation;

/// A boxed diagnostic that can hold any diagnostic type.
pub type BoxedDiagnostic = Box<dyn Diagnostic + Send + Sync + 'static>;

/// Formatter for printing diagnostics.
pub struct DiagnosticPrinter {
    handler: GraphicalReportHandler,
}

impl DiagnosticPrinter {
    #[must_use]
    pub fn plain() -> Self {
        Self { handler: GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor()) }
    }

    #[must_use]
    pub fn fancy() -> Self {
        Self { handler: GraphicalReportHandler::new_themed(GraphicalTheme::unicode()) }
    }

    /// Print a diagnostic to the writer.
    ///
    /// # Errors
    ///
    /// Returns an I/O error if writing failed.
    pub fn print(&self, diagnostic: &dyn Diagnostic, writer: &mut dyn Write) -> io::Result<()> {
        let mut output = String::new();
        self.handler.render_report(&mut output, diagnostic).map_err(io::Error::other)?;
        writeln!(writer, "{output}")
    }
}

#[derive(Debug)]
pub struct UnusedDependency {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    /// Dependency name (for fixing).
    pub name: String,
}

impl fmt::Display for UnusedDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unused dependency `{}`", self.name)
    }
}

impl Error for UnusedDependency {}

impl Diagnostic for UnusedDependency {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::unused"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("remove this dependency"))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("not used anywhere in the code".to_owned()),
            self.span,
        ))))
    }
}

#[derive(Debug)]
pub struct UnusedOptionalDependency {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    pub name: String,
    pub related: Vec<RelatedAdvice>,
}

impl fmt::Display for UnusedOptionalDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unused optional dependency `{}`", self.name)
    }
}

impl Error for UnusedOptionalDependency {}

impl Diagnostic for UnusedOptionalDependency {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::unused_optional"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Warning)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("consider removing this dependency, or suppressing this warning"))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("not used anywhere in the code".to_owned()),
            self.span,
        ))))
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        if self.related.is_empty() {
            None
        } else {
            Some(Box::new(self.related.iter().map(|r| r as &dyn Diagnostic)))
        }
    }
}

#[derive(Debug)]
pub struct MisplacedDependency {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    pub help: String,
    /// Dependency name (for fixing).
    pub name: String,
    /// Dependency location (for fixing).
    pub location: DepLocation,
}

impl fmt::Display for MisplacedDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "misplaced dependency `{}`", self.name)
    }
}

impl Error for MisplacedDependency {}

impl Diagnostic for MisplacedDependency {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::misplaced"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(&self.help))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("only used in dev targets".to_owned()),
            self.span,
        ))))
    }
}

#[derive(Debug)]
pub struct MisplacedOptionalDependency {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    pub help: String,
    pub name: String,
    pub related: Vec<RelatedAdvice>,
}

impl fmt::Display for MisplacedOptionalDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "misplaced optional dependency `{}`", self.name)
    }
}

impl Error for MisplacedOptionalDependency {}

impl Diagnostic for MisplacedOptionalDependency {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::misplaced_optional"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Warning)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(&self.help))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("only used in dev targets".to_owned()),
            self.span,
        ))))
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        if self.related.is_empty() {
            None
        } else {
            Some(Box::new(self.related.iter().map(|r| r as &dyn Diagnostic)))
        }
    }
}

#[derive(Debug)]
pub struct UnusedWorkspaceDependency {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    /// Dependency name (for fixing).
    pub name: String,
}

impl fmt::Display for UnusedWorkspaceDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unused workspace dependency `{}`", self.name)
    }
}

impl Error for UnusedWorkspaceDependency {}

impl Diagnostic for UnusedWorkspaceDependency {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::unused_workspace"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("remove it from `[workspace.dependencies]`"))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some("not used by any package in this workspace".to_owned()),
            self.span,
        ))))
    }
}

#[derive(Debug)]
pub struct RedundantIgnore {
    pub src: NamedSource<String>,
    pub span: (usize, usize),
    pub name: String,
    pub reason: String,
}

impl fmt::Display for RedundantIgnore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redundant ignore `{}`", self.name)
    }
}

impl Error for RedundantIgnore {}

impl Diagnostic for RedundantIgnore {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::redundant_ignore"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Warning)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("remove this from the ignored list"))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.reason.clone()),
            self.span,
        ))))
    }
}

/// Related advice for optional dependency diagnostics.
#[derive(Debug)]
pub enum RelatedAdvice {
    UsedInFeature { src: NamedSource<String>, span: (usize, usize), feature: String },
    BreakingChange,
}

impl fmt::Display for RelatedAdvice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UsedInFeature { .. } => write!(f, "used in feature"),
            Self::BreakingChange => {
                write!(f, "removing an optional dependency is a breaking change")
            }
        }
    }
}

impl Error for RelatedAdvice {}

impl Diagnostic for RelatedAdvice {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            Self::UsedInFeature { src, .. } => Some(src),
            Self::BreakingChange => None,
        }
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            Self::UsedInFeature { span, feature, .. } => {
                Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                    Some(format!("referenced by feature `{feature}`")),
                    *span,
                ))))
            }
            Self::BreakingChange => None,
        }
    }
}

#[derive(Debug)]
pub struct IgnoreHelpPackage;

impl fmt::Display for IgnoreHelpPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress a warning within a package")
    }
}

impl Error for IgnoreHelpPackage {}

impl Diagnostic for IgnoreHelpPackage {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("[package.metadata.cargo-shear]\nignored = [\"crate_name\"]"))
    }
}

#[derive(Debug)]
pub struct IgnoreHelpWorkspace;

impl fmt::Display for IgnoreHelpWorkspace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress a warning across a workspace")
    }
}

impl Error for IgnoreHelpWorkspace {}

impl Diagnostic for IgnoreHelpWorkspace {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("[workspace.metadata.cargo-shear]\nignored = [\"crate_name\"]"))
    }
}

#[derive(Debug)]
pub struct FixHelp;

impl fmt::Display for FixHelp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to automatically fix issues, run with `--fix`")
    }
}

impl Error for FixHelp {}

impl Diagnostic for FixHelp {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }
}

#[derive(Debug)]
pub struct ProcessingError {
    pub message: String,
}

impl ProcessingError {
    pub fn new(err: impl fmt::Display) -> Self {
        Self { message: format!("{err:#}") }
    }
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

impl Error for ProcessingError {}

impl Diagnostic for ProcessingError {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("shear::error"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new("run with `RUST_BACKTRACE=1` environment variable to display a backtrace"))
    }
}
