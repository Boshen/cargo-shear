use std::{io, io::IsTerminal, str::FromStr};

use crate::{diagnostics::ShearAnalysis, output::miette::MietteRenderer};

pub mod json;
pub mod miette;

/// Output format for cargo-shear.
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    /// Auto format with colors and unicode.
    #[default]
    Auto,

    /// JSON format for machine-readable output.
    Json,
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "json" => Ok(Self::Json),
            _ => Err(format!("unknown format: {s}, expected: auto, json")),
        }
    }
}

/// Color mode for output.
#[derive(Debug, Clone, Copy, Default)]
pub enum ColorMode {
    /// Automatically detect based on environment.
    #[default]
    Auto,

    /// Always use colors.
    Always,

    /// Never use colors.
    Never,
}

impl ColorMode {
    /// Whether to show color.
    #[must_use]
    pub fn enabled(self) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => {
                if std::env::var_os("NO_COLOR").is_some() {
                    return false;
                }

                std::io::stdout().is_terminal()
            }
        }
    }
}

impl FromStr for ColorMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            _ => Err(format!("unknown color option: {s}, expected one of: auto, always, never")),
        }
    }
}

pub struct Renderer<W> {
    writer: W,
    format: OutputFormat,
    color: bool,
}

impl<W: io::Write> Renderer<W> {
    pub const fn new(writer: W, format: OutputFormat, color: bool) -> Self {
        Self { writer, format, color }
    }

    pub fn render(&mut self, analysis: &ShearAnalysis) -> io::Result<()> {
        match self.format {
            OutputFormat::Auto => {
                let mut renderer = MietteRenderer::new(&mut self.writer, self.color);
                renderer.render(analysis)
            }
            OutputFormat::Json => {
                let mut renderer = json::JsonRenderer::new(&mut self.writer);
                renderer.render(analysis)
            }
        }
    }
}
