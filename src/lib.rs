//! Library crate for NGDAR — enables integration tests to access public APIs.
#![warn(missing_docs)]

use std::path::Path;

/// Converts a filesystem path to a string using `/` as the separator.
///
/// Repository paths (index entries, tree names, archive members) are stored
/// with forward slashes so an archive written on Windows stays readable on
/// Linux/macOS and vice-versa.
pub fn path_to_slash(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

/// Cache subsystem: TSV-based fingerprint cache (size, mtime, hash).
pub mod cache;
/// CLI argument parsing and subcommand dispatch.
pub mod cli;
/// Command implementations: init, add, status, commit, pack, hash, log, export, db_export.
pub mod commands;
/// Repository metadata: init, open, find, HEAD/index read/write.
pub mod config;
/// Error types and formatting.
pub mod error;
/// BLAKE3 hashing for files, strings, and hex output.
pub mod hash;
/// Cache-aware hash proxy — the single entry point for file hashes.
pub mod hasher;
/// `.ngdarignore` pattern matching and untracked file listing.
pub mod ignore;
/// Content-addressable objects: Meta, Tree, Commit.
pub mod objects;
