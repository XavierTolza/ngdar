//! CLI argument parsing and command definitions.
//!
//! Uses [`clap`] for argument parsing. Defines the top-level [`Cli`] struct
//! and the [`Commands`] enum with all subcommands.

use clap::{Parser, Subcommand};

/// NGDAR - New Generation Disk Archiving
///
/// Git-like incremental archiving for massive binary files.
/// Archives are standard .tar files containing both full metadata history
/// and incremental binary data, suitable for long-term cold storage.
#[derive(Parser, Debug)]
#[command(name = "ngdar", version = "1.0.0", about, long_about = None)]
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

    /// Display the contents of a previously created archive as a table
    ///
    /// Lists all files in the archive alongside their BLAKE3 hash.
    /// For archives created by ngdar, metadata from the .ngdar/objects
    /// entries is parsed and shown.
    ArchiveContent {
        /// Path to the tar archive
        archive: String,
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

    /// Remove all metadata associated with a given volume ID from the database
    ///
    /// Deletes all Meta objects whose volume_id matches the given identifier.
    /// Also updates affected Tree objects by removing entries that reference
    /// the deleted Meta objects.
    ArchiveRemove {
        /// Volume identifier to remove (e.g., "DVD-001")
        #[arg(long = "vol-id")]
        vol_id: String,
    },
}
