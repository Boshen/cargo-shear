use std::{error::Error, fmt, io};

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, Severity,
};
use owo_colors::OwoColorize;

use crate::diagnostics::ShearAnalysis;

/// A renderer using `miette`.
pub struct MietteRenderer<W> {
    writer: W,
    color: bool,
}

impl<W: io::Write> MietteRenderer<W> {
    pub const fn new(writer: W, color: bool) -> Self {
        Self { writer, color }
    }

    pub fn render(&mut self, analysis: &ShearAnalysis) -> io::Result<()> {
        let theme =
            if self.color { GraphicalTheme::unicode() } else { GraphicalTheme::unicode_nocolor() };

        let handler = GraphicalReportHandler::new_themed(theme.clone());
        let mut output = String::new();

        // Print all diagnostics
        for diagnostic in &analysis.findings {
            output.clear();
            handler.render_report(&mut output, diagnostic).map_err(io::Error::other)?;
            writeln!(self.writer, "{output}")?;
        }

        // Print summary
        writeln!(self.writer, "{}", "shear/summary".style(theme.styles.advice))?;
        writeln!(self.writer)?;

        if analysis.findings.is_empty() && analysis.fixed == 0 {
            writeln!(self.writer, "  {} no issues found", "✓".style(theme.styles.advice))?;
            return Ok(());
        }

        if analysis.findings.is_empty() && analysis.fixed > 0 {
            writeln!(
                self.writer,
                "  {} {} issue{} fixed",
                "✓".style(theme.styles.advice),
                analysis.fixed,
                if analysis.fixed == 1 { "" } else { "s" }
            )?;

            return Ok(());
        }

        // Print stats
        let errors = analysis.unused + analysis.misplaced;
        if errors > 0 {
            writeln!(
                self.writer,
                "  {} {} error{}",
                "✗".style(theme.styles.error),
                errors,
                if errors == 1 { "" } else { "s" }
            )?;
        }

        if analysis.warnings > 0 {
            writeln!(
                self.writer,
                "  {} {} warning{}",
                "⚠".style(theme.styles.warning),
                analysis.warnings,
                if analysis.warnings == 1 { "" } else { "s" }
            )?;
        }

        if analysis.fixed > 0 {
            writeln!(
                self.writer,
                "  {} {} issue{} fixed",
                "⚒".style(theme.styles.advice),
                analysis.fixed,
                if analysis.fixed == 1 { "" } else { "s" }
            )?;
        }

        writeln!(self.writer)?;
        writeln!(self.writer, "Advice:")?;

        if errors > 0 && analysis.fixed == 0 {
            writeln!(
                self.writer,
                "  {} run with `--fix` to fix {} issue{}",
                "☞".style(theme.styles.advice),
                errors,
                if errors == 1 { "" } else { "s" }
            )?;
        }

        output.clear();
        handler
            .render_report(&mut output, &PackageIgnoreHelpDiagnostic::new())
            .map_err(io::Error::other)?;

        write!(self.writer, "{output}")?;

        output.clear();
        handler
            .render_report(&mut output, &WorkspaceIgnoreHelpDiagnostic::new())
            .map_err(io::Error::other)?;

        write!(self.writer, "{output}")?;

        Ok(())
    }
}

/// A diagnostic that displays help for ignoring issues at package level.
struct PackageIgnoreHelpDiagnostic {
    source: NamedSource<&'static str>,
}

impl PackageIgnoreHelpDiagnostic {
    const SOURCE: &str = "[package.metadata.cargo-shear]\nignored = [\"crate-name\"]";

    fn new() -> Self {
        Self { source: NamedSource::new("Cargo.toml", Self::SOURCE) }
    }
}

impl fmt::Debug for PackageIgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageIgnoreHelpDiagnostic").finish()
    }
}

impl fmt::Display for PackageIgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress an issue within a package")
    }
}

impl Error for PackageIgnoreHelpDiagnostic {}

impl Diagnostic for PackageIgnoreHelpDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::at(42..54, "add the crate name here"))))
    }
}

/// A diagnostic that displays help for ignoring issues at workspace level.
struct WorkspaceIgnoreHelpDiagnostic {
    source: NamedSource<&'static str>,
}

impl WorkspaceIgnoreHelpDiagnostic {
    const SOURCE: &str = "[workspace.metadata.cargo-shear]\nignored = [\"crate-name\"]";

    fn new() -> Self {
        Self { source: NamedSource::new("Cargo.toml", Self::SOURCE) }
    }
}

impl fmt::Debug for WorkspaceIgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceIgnoreHelpDiagnostic").finish()
    }
}

impl fmt::Display for WorkspaceIgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress an issue across a workspace")
    }
}

impl Error for WorkspaceIgnoreHelpDiagnostic {}

impl Diagnostic for WorkspaceIgnoreHelpDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::at(44..56, "add the crate name here"))))
    }
}
