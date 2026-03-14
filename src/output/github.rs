use std::io;

use miette::{Diagnostic, Severity, SourceCode};

use crate::diagnostics::{ShearAnalysis, ShearDiagnostic};

/// GitHub Actions workflow commands renderer.
///
/// Outputs diagnostics in the format:
/// `::error file={file},line={line},title={code}::{message}`
///
/// See <https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-commands>
pub struct GitHubRenderer<W> {
    writer: W,
}

impl<W: io::Write> GitHubRenderer<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn render(&mut self, analysis: &ShearAnalysis) -> io::Result<()> {
        for diagnostic in &analysis.findings {
            self.render_diagnostic(diagnostic)?;
        }
        Ok(())
    }

    fn render_diagnostic(&mut self, diagnostic: &ShearDiagnostic) -> io::Result<()> {
        let level = match diagnostic.kind.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Advice => "notice",
        };

        let code = diagnostic.kind.code();
        let message = diagnostic.kind.message();

        write!(self.writer, "::{level} ")?;

        let mut needs_comma = false;
        if let Some(source) = &diagnostic.source {
            write!(self.writer, "file={}", source.name())?;
            needs_comma = true;

            if let Some(span) = diagnostic.span
                && let Ok(contents) = source.read_span(&span, 0, 0)
            {
                let line = contents.line() + 1;
                let col = contents.column() + 1;
                write!(self.writer, ",line={line},col={col}")?;
            }
        }

        if needs_comma {
            write!(self.writer, ",")?;
        }
        write!(self.writer, "title={code}::{message}")?;

        if let Some(help) = diagnostic.help() {
            write!(self.writer, " ({help})")?;
        }

        writeln!(self.writer)?;
        Ok(())
    }
}
