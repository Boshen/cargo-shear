use std::io;

use miette::{Diagnostic, Severity};

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
            Severity::Warning | Severity::Advice => "warning",
        };

        let code = diagnostic.kind.code();
        let message = diagnostic.kind.message();

        let mut properties = Vec::new();

        if let Some(source) = &diagnostic.source {
            properties.push(format!("file={}", source.name()));

            if let Some(span) = diagnostic.span {
                let source_content: &str = source.inner();
                let (line, col) = offset_to_line_col(source_content, span.offset());
                properties.push(format!("line={line}"));
                properties.push(format!("col={col}"));
            }
        }

        properties.push(format!("title={code}"));

        let help_suffix = diagnostic.help().map(|h| format!(" ({h})")).unwrap_or_default();

        writeln!(self.writer, "::{level} {}::{message}{help_suffix}", properties.join(","))?;

        Ok(())
    }
}

/// Convert a byte offset to a 1-based line and column number.
fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
