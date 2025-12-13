use std::io;

use serde::{Deserialize, Serialize};

use crate::diagnostics::ShearAnalysis;

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
        serde_json::to_writer_pretty(&mut self.writer, &output)
            .map_err(io::Error::other)?;
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

    /// Optional fix that can be applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

impl Finding {
    fn from_diagnostic(diagnostic: &crate::diagnostics::ShearDiagnostic) -> Self {
        use crate::diagnostics::DiagnosticKind;
        use miette::Diagnostic;

        let kind = diagnostic.kind();
        let code = kind.code().to_owned();

        let severity = match kind.severity() {
            miette::Severity::Error => "error",
            miette::Severity::Warning => "warning",
            miette::Severity::Advice => "advice",
        }
        .to_owned();

        let message = kind.message();

        let file = diagnostic.file_name().map(std::borrow::ToOwned::to_owned);

        let location =
            diagnostic.span().map(|span| Location { offset: span.offset(), length: span.len() });

        let help = diagnostic.help().map(|h| h.to_string());

        // Generate a fix suggestion based on the diagnostic kind
        // Only fixable issues get a fix object
        let fix = match kind {
            DiagnosticKind::UnusedDependency { .. }
            | DiagnosticKind::UnusedWorkspaceDependency { .. }
            | DiagnosticKind::MisplacedDependency { .. } => {
                help.as_ref().map(|h| Fix { description: h.clone() })
            }
            DiagnosticKind::UnusedOptionalDependency { .. }
            | DiagnosticKind::UnusedFeatureDependency { .. }
            | DiagnosticKind::MisplacedOptionalDependency { .. }
            | DiagnosticKind::UnlinkedFiles { .. }
            | DiagnosticKind::EmptyFiles { .. }
            | DiagnosticKind::UnknownIgnore { .. }
            | DiagnosticKind::RedundantIgnore { .. }
            | DiagnosticKind::RedundantIgnorePath { .. } => None,
        };

        Self { code, severity, message, file, location, help, fix }
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

/// A fix that can be applied to resolve the issue.
#[derive(Debug, Serialize, Deserialize)]
pub struct Fix {
    /// Description of what the fix does.
    pub description: String,
}
