mod cli;
mod engine;
mod logger;
mod server;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(subcommand)]
    pub mode: RunMode,
}

#[derive(Debug, Subcommand)]
pub enum RunMode {
    /// Run as cli
    Cli,
    /// Run as web server
    Server,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init("searxiv.log")?;

    let cli = Cli::parse();

    match cli.mode {
        RunMode::Cli => cli::run_cli().await,
        RunMode::Server => server::run_server().await,
    }
}
