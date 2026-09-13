//! CLI argument parsing and command definitions.
//!
//! Uses clap for argument parsing. Defines the top-level Cli struct
//! and the Commands enum with all subcommands.

use clap::{Parser, Subcommand};

/// NGDAR - New Generation Disk Archiving
///
/// Git-like incremental archiving for massive binary files.
/// Archives are standard .tar files containing both full metadata history
/// and incremental binary data, suitable for long-term cold storage.
#[derive(Parser, Debug)]
#[command(name = "ngdar", version, about, long_about = None)]
pub struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

/// All available NGDAR subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new ngdar repository in the current directory
    Init,

    /// Show repository status (staged, unstaged, untracked)
    Status,

    /// Add files to the staging area (index)
    Add {
        /// File paths to add
        #[arg(required = true)]
        paths: Vec<String>,
    },

    /// Create a TAR archive from staged files
    Pack {
        /// Volume identifier (e.g., "DVD-001", "ARCHIVE-2026-08")
        #[arg(long = "vol-id")]
        vol_id: String,

        /// Output tar file path
        #[arg(long = "out")]
        out: String,

        /// Commit message
        #[arg(short = 'm')]
        message: String,
    },

    /// Compute the BLAKE3 hash of a file
    Hash {
        /// Path to the file to hash
        path: String,
    },

    /// List commits or show files in a commit
    ///
    /// Without arguments, walks the commit chain and shows all commits.
    /// With a commit hash, lists all files in that commit with their
    /// path, BLAKE3 hash, and size.
    Log {
        /// Optional commit hash to inspect
        commit_hash: Option<String>,
    },

    /// Recreate a TAR archive from stored metadata
    ///
    /// Reads files from disk at their original paths, verifies their
    /// BLAKE3 hash, and packs them into a tar archive. Useful for
    /// restoring archives that were lost.
    Export {
        /// Commit hash to export
        commit_hash: String,

        /// Output tar file path
        #[arg(long = "out")]
        out: String,
    },

    /// Export all metadata from the repository database to a CSV file
    ///
    /// Exports every object (Meta, Tree, Commit) stored in .ngdar/objects/
    /// as rows in a CSV file with columns such as type, hash, size, mtime,
    /// permissions, binary_hash, volume_id, timestamp, and message.
    DbExport {
        /// Output CSV file path
        csv: String,
    },
}
