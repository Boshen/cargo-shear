//! Error handling for cargo-shear.
//!
//! This module provides custom error types that capture detailed context
//! about failures during dependency analysis. All errors are boxed to reduce
//! the size of `Result` types throughout the codebase.

use std::fmt;
use std::io;
use std::path::PathBuf;
use std::backtrace::Backtrace;

/// The main error type for cargo-shear operations.
///
/// This error type captures both the error kind and a backtrace for debugging.
/// Errors are typically boxed when returned to reduce Result size.
pub struct Error {
    /// The specific kind of error that occurred
    kind: ErrorKind,
    /// Backtrace captured at the point of error creation
    backtrace: Backtrace,
}

impl Error {
    /// Get the backtrace associated with this error.
    ///
    /// The backtrace is captured at the point where the error was created.
    pub const fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    /// Create a new error with the given kind.
    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }

    /// Create an I/O error.
    pub fn io(e: io::Error) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Io(e)))
    }

    /// Create a metadata error with a custom message.
    pub fn metadata(msg: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Metadata(msg)))
    }

    /// Create a parse error with a custom message.
    pub fn parse(msg: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Parse(msg)))
    }

    /// Create an error for cargo expand failures.
    pub fn expand(target: String, message: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Expand { target, message }))
    }

    /// Create an error for missing parent paths.
    pub fn missing_parent(path: PathBuf) -> Box<Self> {
        Box::new(Self::new(ErrorKind::MissingParent(path)))
    }

    /// Create an error for packages that cannot be found.
    pub fn package_not_found(name: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::PackageNotFound(name)))
    }
}

/// Specific error kinds that can occur during cargo-shear operations.
#[derive(Debug)]
pub enum ErrorKind {
    /// I/O error (file operations, etc.)
    Io(io::Error),
    /// Error related to cargo metadata parsing
    Metadata(String),
    /// Error parsing package IDs or other strings
    Parse(String),
    /// Error during cargo expand operation
    Expand {
        /// The target that failed to expand
        target: String,
        /// The error message from cargo expand
        message: String
    },
    /// Failed to get parent path of a file
    MissingParent(PathBuf),
    /// Package not found in workspace
    PackageNotFound(String),
    /// Error editing TOML files
    TomlEdit(toml_edit::TomlError),
    /// Error parsing Cargo.toml files
    CargoToml(cargo_toml::Error),
    /// Syntax error in Rust source code
    Syn(syn::Error),
    /// UTF-8 conversion error
    Utf8(std::string::FromUtf8Error),
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Io(e) => write!(f, "IO error: {e}"),
            ErrorKind::Metadata(msg) => write!(f, "Metadata error: {msg}"),
            ErrorKind::Parse(msg) => write!(f, "Parse error: {msg}"),
            ErrorKind::Expand { target, message } => {
                write!(f, "Cargo expand failed for {target}: {message}")
            }
            ErrorKind::MissingParent(path) => {
                write!(f, "Failed to get parent path: {}", path.display())
            }
            ErrorKind::PackageNotFound(name) => write!(f, "Package not found: {name}"),
            ErrorKind::TomlEdit(e) => write!(f, "TOML edit error: {e}"),
            ErrorKind::CargoToml(e) => write!(f, "Cargo TOML error: {e}"),
            ErrorKind::Syn(e) => write!(f, "Syntax error: {e}"),
            ErrorKind::Utf8(e) => write!(f, "UTF-8 conversion error: {e}"),
        }
    }
}

impl std::error::Error for Error {}

#[allow(clippy::use_self, reason = "Box<Self> doesn't work correctly here")]
impl From<io::Error> for Box<Error> {
    fn from(e: io::Error) -> Self {
        Box::new(Error::new(ErrorKind::Io(e)))
    }
}

#[allow(clippy::use_self, reason = "Box<Self> doesn't work correctly here")]
impl From<toml_edit::TomlError> for Box<Error> {
    fn from(e: toml_edit::TomlError) -> Self {
        Box::new(Error::new(ErrorKind::TomlEdit(e)))
    }
}

#[allow(clippy::use_self, reason = "Box<Self> doesn't work correctly here")]
impl From<cargo_toml::Error> for Box<Error> {
    fn from(e: cargo_toml::Error) -> Self {
        Box::new(Error::new(ErrorKind::CargoToml(e)))
    }
}

#[allow(clippy::use_self, reason = "Box<Self> doesn't work correctly here")]
impl From<syn::Error> for Box<Error> {
    fn from(e: syn::Error) -> Self {
        Box::new(Error::new(ErrorKind::Syn(e)))
    }
}

#[allow(clippy::use_self, reason = "Box<Self> doesn't work correctly here")]
impl From<std::string::FromUtf8Error> for Box<Error> {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Box::new(Error::new(ErrorKind::Utf8(e)))
    }
}

/// Convenient type alias for Results using our custom Error type.
///
/// All errors are boxed to reduce the size of Result types throughout the codebase.
pub type Result<T> = std::result::Result<T, Box<Error>>;