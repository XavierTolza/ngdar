//! Library crate for NGDAR — enables integration tests to access public APIs.
#![warn(missing_docs)]

/// Cache subsystem: TSV-based fingerprint cache (size, mtime, hash).
pub mod cache;
/// CLI argument parsing and subcommand dispatch.
pub mod cli;
/// Command implementations: init, add, status, pack, hash, log, export, db_export.
pub mod commands;
/// Repository metadata: init, open, find, HEAD/index read/write.
pub mod config;
/// Error types and formatting.
pub mod error;
/// BLAKE3 hashing for files, strings, and hex output.
pub mod hash;
/// `.ngdarignore` pattern matching and untracked file listing.
pub mod ignore;
/// Content-addressable objects: Meta, Tree, Commit.
pub mod objects;
