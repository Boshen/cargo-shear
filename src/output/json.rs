use std::io;

use miette::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

use crate::diagnostics::{ShearAnalysis, ShearDiagnostic};

/// JSON renderer for cargo-shear output.
pub struct JsonRenderer<W> {
    writer: W,
}

impl<W: io::Write> JsonRenderer<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn render(&mut self, analysis: &ShearAnalysis) -> io::Result<()> {
        let output = JsonOutput::from_analysis(analysis);
        serde_json::to_writer_pretty(&mut self.writer, &output).map_err(io::Error::other)?;
        writeln!(self.writer)?;
        Ok(())
    }
}

/// Root JSON output structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
    /// Summary statistics.
    pub summary: Summary,

    /// List of all findings.
    pub findings: Vec<Finding>,
}

impl JsonOutput {
    fn from_analysis(analysis: &ShearAnalysis) -> Self {
        Self {
            summary: Summary {
                errors: analysis.errors,
                warnings: analysis.warnings,
                fixed: analysis.fixed,
            },
            findings: analysis.findings.iter().map(Finding::from_diagnostic).collect(),
        }
    }
}

/// Summary statistics.
#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    /// Number of errors found.
    pub errors: usize,

    /// Number of warnings found.
    pub warnings: usize,

    /// Number of issues fixed (only when --fix is used).
    pub fixed: usize,
}

/// A single finding/diagnostic.
#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    /// The diagnostic code (e.g., "`shear/unused_dependency`").
    pub code: String,

    /// The severity level: "error" or "warning".
    pub severity: String,

    /// Human-readable message describing the issue.
    pub message: String,

    /// Optional file path where the issue was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,

    /// Optional location within the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,

    /// Optional help/suggestion text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,

    /// Whether this issue can be automatically fixed with `--fix`.
    pub fixable: bool,
}

impl Finding {
    fn from_diagnostic(diagnostic: &ShearDiagnostic) -> Self {
        let code = diagnostic.kind.code().to_owned();

        let severity = match diagnostic.kind.severity() {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Advice => "advice",
        }
        .to_owned();

        let message = diagnostic.kind.message();

        let file = diagnostic.source.as_ref().map(|s| s.name().to_owned());

        let location =
            diagnostic.span.map(|span| Location { offset: span.offset(), length: span.len() });

        let help = diagnostic.help().map(|h| h.to_string());

        let fixable = diagnostic.kind.is_fixable();

        Self { code, severity, message, file, location, help, fixable }
    }
}

/// Location information within a file.
#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    /// Byte offset from the start of the file.
    pub offset: usize,

    /// Length in bytes.
    pub length: usize,
}
