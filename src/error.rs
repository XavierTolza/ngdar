use std::fmt;
use std::io;

#[derive(Debug)]
pub enum NgdarError {
    NotARepository,
    Io(io::Error),
    Walkdir(walkdir::Error),
    Hash(String),
    Parse(String),
    Index(String),
    Cache(String),
    Config(String),
    Other(String),
}

impl fmt::Display for NgdarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NgdarError::NotARepository => write!(f, "Not an ngdar repository (or any parent): .ngdar"),
            NgdarError::Io(e) => write!(f, "IO error: {}", e),
            NgdarError::Walkdir(e) => write!(f, "Walkdir error: {}", e),
            NgdarError::Hash(s) => write!(f, "Hash error: {}", s),
            NgdarError::Parse(s) => write!(f, "Parse error: {}", s),
            NgdarError::Index(s) => write!(f, "Index error: {}", s),
            NgdarError::Cache(s) => write!(f, "Cache error: {}", s),
            NgdarError::Config(s) => write!(f, "Config error: {}", s),
            NgdarError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for NgdarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NgdarError::Io(e) => Some(e),
            NgdarError::Walkdir(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NgdarError {
    fn from(e: io::Error) -> Self {
        NgdarError::Io(e)
    }
}

impl From<walkdir::Error> for NgdarError {
    fn from(e: walkdir::Error) -> Self {
        NgdarError::Walkdir(e)
    }
}