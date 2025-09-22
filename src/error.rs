use std::fmt;
use std::io;
use std::path::PathBuf;
use std::backtrace::Backtrace;

pub struct Error {
    kind: ErrorKind,
    backtrace: Backtrace,
}

impl Error {
    pub const fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }

    fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            backtrace: Backtrace::capture(),
        }
    }

    pub fn io(e: io::Error) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Io(e)))
    }

    pub fn metadata(msg: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Metadata(msg)))
    }

    pub fn parse(msg: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Parse(msg)))
    }

    pub fn expand(target: String, message: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::Expand { target, message }))
    }

    pub fn missing_parent(path: PathBuf) -> Box<Self> {
        Box::new(Self::new(ErrorKind::MissingParent(path)))
    }

    pub fn package_not_found(name: String) -> Box<Self> {
        Box::new(Self::new(ErrorKind::PackageNotFound(name)))
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    Io(io::Error),
    Metadata(String),
    Parse(String),
    Expand { target: String, message: String },
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

pub type Result<T> = std::result::Result<T, Box<Error>>;