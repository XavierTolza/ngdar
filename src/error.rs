//! Error types for NGDAR operations.
//!
//! Defines [`NgdarError`], a unified error enum that wraps I/O, parsing,
//! filesystem-walk, and domain-specific errors with automatic conversions.

use std::fmt;
use std::io;

/// Unified error type for all NGDAR operations.
///
/// Provides automatic conversion from [`io::Error`] and [`walkdir::Error`]
/// via [`From`] implementations.
#[derive(Debug)]
pub enum NgdarError {
    /// No `.ngdar` directory found in the current or any parent directory.
    NotARepository,
    /// Wraps an I/O error.
    Io(io::Error),
    /// Wraps a filesystem walk error.
    Walkdir(walkdir::Error),
    /// A hashing-related error with a message.
    Hash(String),
    /// A parsing error with a message.
    Parse(String),
    /// A configuration error with a message.
    Config(String),
    /// Any other error with a message.
    Other(String),
}

impl fmt::Display for NgdarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NgdarError::NotARepository => {
                write!(f, "Not an ngdar repository (or any parent): .ngdar")
            }
            NgdarError::Io(e) => write!(f, "IO error: {}", e),
            NgdarError::Walkdir(e) => write!(f, "Walkdir error: {}", e),
            NgdarError::Hash(s) => write!(f, "Hash error: {}", s),
            NgdarError::Parse(s) => write!(f, "Parse error: {}", s),
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
