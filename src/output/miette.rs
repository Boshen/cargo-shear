use std::{error::Error, fmt, io, ops::Range};

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, Severity,
};
use owo_colors::OwoColorize;

use crate::diagnostics::ShearAnalysis;

/// Default human-readable renderer: writes `miette` graphical reports for each
/// diagnostic, then a styled summary line and any advice snippets.
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

        // Render each diagnostic into a reusable buffer.
        for diagnostic in &analysis.findings {
            output.clear();
            handler.render_report(&mut output, diagnostic).map_err(io::Error::other)?;
            writeln!(self.writer, "{output}")?;
        }

        // Summary header (styled like `miette`'s "advice" diagnostics).
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
                plural(analysis.fixed)
            )?;

            return Ok(());
        }

        // Per-severity counters that make up the summary block.
        if analysis.errors > 0 {
            writeln!(
                self.writer,
                "  {} {} error{}",
                "✗".style(theme.styles.error),
                analysis.errors,
                plural(analysis.errors)
            )?;
        }

        if analysis.warnings > 0 {
            writeln!(
                self.writer,
                "  {} {} warning{}",
                "⚠".style(theme.styles.warning),
                analysis.warnings,
                plural(analysis.warnings)
            )?;
        }

        if analysis.fixed > 0 {
            writeln!(
                self.writer,
                "  {} {} issue{} fixed",
                "⚒".style(theme.styles.advice),
                analysis.fixed,
                plural(analysis.fixed)
            )?;
        }

        let show_expand = analysis.show_expand && !analysis.options.expand;
        let show_fix = analysis.show_fix && !analysis.options.fix;

        let has_advice =
            show_expand || show_fix || analysis.show_ignored || analysis.show_ignored_paths;

        if !has_advice {
            return Ok(());
        }

        writeln!(self.writer)?;
        writeln!(self.writer, "Advice:")?;

        if show_expand {
            writeln!(
                self.writer,
                "  {} run with `--expand` for more accurate results",
                "☞".style(theme.styles.advice),
            )?;
        }

        if show_fix {
            writeln!(
                self.writer,
                "  {} run with `--fix` to fix {} issue{}",
                "☞".style(theme.styles.advice),
                analysis.fixable,
                plural(analysis.fixable)
            )?;
        }

        if analysis.show_ignored {
            output.clear();
            handler
                .render_report(&mut output, &SnippetHelpDiagnostic::ignored())
                .map_err(io::Error::other)?;

            write!(self.writer, "{output}")?;
        }

        if analysis.show_ignored_paths {
            output.clear();
            handler
                .render_report(&mut output, &SnippetHelpDiagnostic::ignored_paths())
                .map_err(io::Error::other)?;

            write!(self.writer, "{output}")?;
        }

        Ok(())
    }
}

const fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// An advice diagnostic showing a `Cargo.toml` snippet for suppressing an issue.
#[derive(Debug)]
struct SnippetHelpDiagnostic {
    message: &'static str,
    source: NamedSource<&'static str>,
    label: &'static str,
    /// Byte range within `source` that `label` points at.
    span: Range<usize>,
}

impl SnippetHelpDiagnostic {
    /// Help for ignoring dependency issues.
    fn ignored() -> Self {
        Self::new(
            "to suppress a dependency issue",
            "[package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]\nignored = [\"crate-name\"]",
            "add a crate name here",
            80..92,
        )
    }

    /// Help for ignoring unlinked/empty file issues.
    fn ignored_paths() -> Self {
        Self::new(
            "to suppress a file issue",
            "[package.metadata.cargo-shear] # or [workspace.metadata.cargo-shear]\nignored-paths = [\"tests/compile/*.rs\"]",
            "add a file pattern here",
            86..106,
        )
    }

    fn new(
        message: &'static str,
        source: &'static str,
        label: &'static str,
        span: Range<usize>,
    ) -> Self {
        Self { message, source: NamedSource::new("Cargo.toml", source), label, span }
    }
}

impl fmt::Display for SnippetHelpDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Error for SnippetHelpDiagnostic {}

impl Diagnostic for SnippetHelpDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Advice)
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::at(self.span.clone(), self.label))))
    }
}
