use std::fmt;
use std::io;
use std::path::PathBuf;
use std::backtrace::Backtrace;

pub struct Error {
    kind: ErrorKind,
    backtrace: Backtrace,
}

impl Error {
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }

    pub fn io(e: io::Error) -> Self {
        Self::new(ErrorKind::Io(e))
    }

    pub fn metadata(msg: String) -> Self {
        Self::new(ErrorKind::Metadata(msg))
    }

    pub fn parse(msg: String) -> Self {
        Self::new(ErrorKind::Parse(msg))
    }

    pub fn expand(target: String, message: String) -> Self {
        Self::new(ErrorKind::Expand { target, message })
    }

    pub fn invalid_path(path: PathBuf) -> Self {
        Self::new(ErrorKind::InvalidPath(path))
    }

    pub fn missing_parent(path: PathBuf) -> Self {
        Self::new(ErrorKind::MissingParent(path))
    }

    pub fn package_not_found(name: String) -> Self {
        Self::new(ErrorKind::PackageNotFound(name))
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    Io(io::Error),
    Metadata(String),
    Parse(String),
    Expand { target: String, message: String },
    InvalidPath(PathBuf),
    MissingParent(PathBuf),
    PackageNotFound(String),
    TomlEdit(toml_edit::TomlError),
    CargoToml(cargo_toml::Error),
    Syn(syn::Error),
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
            ErrorKind::Io(e) => write!(f, "IO error: {}", e),
            ErrorKind::Metadata(msg) => write!(f, "Metadata error: {}", msg),
            ErrorKind::Parse(msg) => write!(f, "Parse error: {}", msg),
            ErrorKind::Expand { target, message } => {
                write!(f, "Cargo expand failed for {}: {}", target, message)
            }
            ErrorKind::InvalidPath(path) => {
                write!(f, "Invalid path: {}", path.display())
            }
            ErrorKind::MissingParent(path) => {
                write!(f, "Failed to get parent path: {}", path.display())
            }
            ErrorKind::PackageNotFound(name) => write!(f, "Package not found: {}", name),
            ErrorKind::TomlEdit(e) => write!(f, "TOML edit error: {}", e),
            ErrorKind::CargoToml(e) => write!(f, "Cargo TOML error: {}", e),
            ErrorKind::Syn(e) => write!(f, "Syntax error: {}", e),
            ErrorKind::Utf8(e) => write!(f, "UTF-8 conversion error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::new(ErrorKind::Io(e))
    }
}

impl From<toml_edit::TomlError> for Error {
    fn from(e: toml_edit::TomlError) -> Self {
        Error::new(ErrorKind::TomlEdit(e))
    }
}

impl From<cargo_toml::Error> for Error {
    fn from(e: cargo_toml::Error) -> Self {
        Error::new(ErrorKind::CargoToml(e))
    }
}

impl From<syn::Error> for Error {
    fn from(e: syn::Error) -> Self {
        Error::new(ErrorKind::Syn(e))
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::new(ErrorKind::Utf8(e))
    }
}

pub type Result<T> = std::result::Result<T, Error>;