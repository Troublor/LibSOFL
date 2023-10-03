mod collect_contracts;
use std::{fs::OpenOptions, path::PathBuf, process};

use crate::collect_contracts::collect_contracts;
use clap::{Parser, Subcommand};
use libsofl::config::flags::SoflConfig;
use tracing::error;
use tracing_subscriber::{
    fmt::layer, prelude::*, registry::Registry, EnvFilter,
};

#[derive(Parser)]
#[command(name = "Knowledge Collector")]
#[command(about = "Extracting knowledge from blockchain history")]
struct Cli {
    /// Number of jobs to run in parallel
    #[arg(short, long)]
    jobs: u32,

    /// Start block number (included)
    #[arg(long, default_value = "0")]
    from: u32,

    /// End block number (excluded)
    #[arg(long)]
    to: u32,

    /// Path to reth database (datadir)
    #[arg(long, default_value = None)]
    datadir: Option<String>,

    /// Database connection URI
    #[arg(long, default_value = None)]
    database: Option<String>,

    /// file to output logs (optional)
    #[arg(long, default_value = None)]
    log_file: Option<PathBuf>,

    /// log level
    #[arg(short, long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect contracts and its invocations
    Contract {},
}

pub fn main() {
    let cli = Cli::parse();

    // build SoflConfig
    let mut cfg = SoflConfig::load().unwrap_or_else(|e| {
        error!(error = format!("{}", e), "failed to load LibSOFL config");
        process::exit(1);
    });
    if let Some(d) = cli.datadir {
        cfg.reth.datadir = d;
    }
    if let Some(d) = cli.database {
        cfg.database.url = d;
    }

    // build logger
    let level_layer = EnvFilter::new(cli.log_level);
    if let Some(f) = cli.log_file {
        let console_layer = layer().with_target(false);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(f)
            .unwrap_or_else(|e| {
                error!(error = format!("{}", e), "failed to open log file");
                process::exit(1);
            });
        let file_layer = layer().with_writer(file);
        let subscriber = Registry::default()
            .with(level_layer)
            .with(console_layer)
            .with(file_layer);
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    } else {
        let console_layer = layer().with_target(false);
        let subscriber =
            Registry::default().with(level_layer).with(console_layer);
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    // start working
    let task = match cli.command {
        Commands::Contract {} => {
            collect_contracts(cli.from, cli.to, cli.jobs, cfg)
        }
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(task);
}
