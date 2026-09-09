mod add;
mod map;

use crate::add::AddCommand;
use crate::map::MapCommand;
use clap::{Parser, Subcommand};
use std::process::exit;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    if opt.verbose {
        configure_logger();
    }

    let result = match opt.command {
        Commands::Map(cmd) => cmd.run().await,
        Commands::Add(cmd) => cmd.run().await,
    };

    match &result {
        Ok(_) => {
            tracing::info!("program finished successfully");
        }
        Err(e) => {
            eprintln!("error: {:?}", e);
        }
    }
    result.map(|_| exit(0)).map_err(|_| exit(1))
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Opt {
    #[command(subcommand)]
    pub command: Commands,

    #[clap(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generating a mapping between two schema registry contexts
    Map(MapCommand),

    /// Configure a schema registry context
    Add(AddCommand),
}

/// Logger for humans. It enables some debug info
fn configure_logger() {
    let format = tracing_subscriber::fmt::layer()
        .with_level(true)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false);
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("debug,hyper=warn"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter)
        .with(format)
        .init();
}
