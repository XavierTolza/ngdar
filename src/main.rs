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

fn main() {
    let cli = Cli::parse();

    let result = match &cli.command {
        Commands::Init => commands::init(),
        Commands::Status => commands::status(),
        Commands::Add { paths } => commands::add(paths),
        Commands::Pack { vol_id, out, message } => {
            commands::pack(vol_id, out, message)
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
