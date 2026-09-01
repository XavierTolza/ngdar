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

mod cache;
mod cli;
mod commands;
mod config;
mod error;
mod hash;
mod ignore;
mod objects;

use clap::Parser;
use cli::{Cli, Commands};

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
        } => commands::pack(vol_id, out, message),
        Commands::Hash { path } => commands::hash(path),
        Commands::ArchiveContent { archive } => commands::archive_content(archive),
        Commands::DbExport { csv } => commands::db_export(csv),
        Commands::ArchiveRemove { vol_id } => commands::archive_remove(vol_id),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
