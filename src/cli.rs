use clap::{Parser, Subcommand};

/// NGDAR - New Generation Disk Archiving
///
/// Git-like incremental archiving for massive binary files.
/// Archives are standard .tar files containing both full metadata history
/// and incremental binary data, suitable for long-term cold storage.
#[derive(Parser, Debug)]
#[command(name = "ngdar", version = "1.0.0", about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

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
}
