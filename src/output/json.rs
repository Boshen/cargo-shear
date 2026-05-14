use std::io;

use miette::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

use crate::diagnostics::{ShearAnalysis, ShearDiagnostic};

/// Pretty-prints a [`ShearAnalysis`] as JSON. The schema is the public
/// `--format=json` contract documented in the README.
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

/// Top-level JSON object emitted by `--format=json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
    /// Aggregate counts (errors, warnings, fixed).
    pub summary: Summary,

    /// One entry per diagnostic, in emission order.
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

/// Aggregate counts shown in the JSON `summary` field.
#[derive(Debug, Serialize, Deserialize)]
pub struct Summary {
    /// Error-severity findings.
    pub errors: usize,

    /// Non-error findings (warnings + advice).
    pub warnings: usize,

    /// Findings actually rewritten on disk (only non-zero with `--fix`).
    pub fixed: usize,
}

/// One JSON-rendered diagnostic.
#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    /// Stable diagnostic code (e.g. `shear/unused_dependency`).
    pub code: String,

    /// One of `error`, `warning`, `advice`.
    pub severity: String,

    /// Human-readable message describing the issue.
    pub message: String,

    /// Path of the file the diagnostic points into (typically a `Cargo.toml`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,

    /// Byte range within `file` to highlight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,

    /// Suggested fix text shown alongside the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,

    /// Whether `--fix` could repair this diagnostic automatically.
    pub fixable: bool,
}

/// Byte range inside a file, used by `Finding::location`.
#[derive(Debug, Serialize, Deserialize)]
pub struct Location {
    /// Byte offset from the start of the file.
    pub offset: usize,

    /// Length in bytes (a zero-length span points just before `offset`).
    pub length: usize,
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
