//! NGDAR — New Generation Disk Archiving
//!
//! A Git-like incremental archiving tool for massive binary files.
//! Archives are standard `.tar` files containing both full metadata history
//! and incremental binary data, suitable for long-term cold storage.
//!
//! # Architecture
//!
//! - **Content-addressable storage** via BLAKE3 hashes
//! - **Plain-text metadata** (Meta, Tree, Commit objects)
//! - **Self-contained archives** with complete history
//!
//! # CLI Commands
//!
//! See [`cli::Commands`] for the full command-line interface.
#![warn(missing_docs)]

use clap::Parser;
use ngdar::cli::{Cli, Commands};
use ngdar::commands;

/// Program entry point.
///
/// Parses CLI arguments and dispatches to the appropriate command handler.
/// Exits with code 1 on error.
fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init => commands::init(),
        Commands::Status => commands::status(),
        Commands::Add { paths } => commands::add(paths),
        Commands::Pack {
            vol_id,
            out,
            message,
            verbose,
        } => commands::pack(vol_id, out, message, *verbose),
        Commands::Hash { path } => commands::hash(path),
        Commands::Log { commit_hash } => commands::log(commit_hash.as_deref()),
        Commands::Export { commit_hash, out } => commands::export(commit_hash, out),
        Commands::DbExport { csv } => commands::db_export(csv),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
