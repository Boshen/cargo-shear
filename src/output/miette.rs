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
        if analysis.errors > 0 {
            writeln!(
                self.writer,
                "  {} {} error{}",
                "✗".style(theme.styles.error),
                analysis.errors,
                if analysis.errors == 1 { "" } else { "s" }
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

        if !analysis.show_fix() && !analysis.show_ignored && !analysis.show_ignored_paths {
            return Ok(());
        }

        writeln!(self.writer)?;
        writeln!(self.writer, "Advice:")?;

        if analysis.show_fix() {
            writeln!(
                self.writer,
                "  {} run with `--fix` to fix {} issue{}",
                "☞".style(theme.styles.advice),
                analysis.errors,
                if analysis.errors == 1 { "" } else { "s" }
            )?;
        }

        if analysis.show_ignored {
            output.clear();
            handler
                .render_report(&mut output, &IgnoreHelpDiagnostic::new())
                .map_err(io::Error::other)?;

            write!(self.writer, "{output}")?;
        }

        if analysis.show_ignored_paths {
            output.clear();
            handler
                .render_report(&mut output, &IgnorePathHelpDiagnostic::new())
                .map_err(io::Error::other)?;

            write!(self.writer, "{output}")?;
        }

        Ok(())
    }
}

/// A diagnostic that displays help for ignoring dependency issues.
struct IgnoreHelpDiagnostic {
    source: NamedSource<&'static str>,
}

impl IgnoreHelpDiagnostic {
    const SOURCE: &str = "[package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]\nignored = [\"crate-name\"]";

    fn new() -> Self {
        Self { source: NamedSource::new("Cargo.toml", Self::SOURCE) }
    }
}

impl fmt::Debug for IgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IgnoreHelpDiagnostic").finish()
    }
}

impl fmt::Display for IgnoreHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress a dependency issue")
    }
}

impl Error for IgnoreHelpDiagnostic {}

impl Diagnostic for IgnoreHelpDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::at(80..92, "add a crate name here"))))
    }
}

/// A diagnostic that displays help for ignoring unlinked file issues.
struct IgnorePathHelpDiagnostic {
    source: NamedSource<&'static str>,
}

impl IgnorePathHelpDiagnostic {
    const SOURCE: &str = "[package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]\nignored-paths = [\"tests/compile/*.rs\"]";

    fn new() -> Self {
        Self { source: NamedSource::new("Cargo.toml", Self::SOURCE) }
    }
}

impl fmt::Debug for IgnorePathHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IgnorePathHelpDiagnostic").finish()
    }
}

impl fmt::Display for IgnorePathHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "to suppress a file issue")
    }
}

impl Error for IgnorePathHelpDiagnostic {}

impl Diagnostic for IgnorePathHelpDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::at(86..106, "add a file pattern here"))))
    }
}
