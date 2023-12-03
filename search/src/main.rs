mod cli;
mod config;
mod engine;
mod server;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[clap(subcommand)]
    pub mode: RunMode,
}

#[derive(Debug, Subcommand)]
pub enum RunMode {
    /// Run as cli
    Cli(Flags),
    /// Run as web server
    Server(Flags),
}

#[derive(Parser, Debug)]
pub struct Flags {
    #[arg(short, long)]
    pub prune: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.mode {
        RunMode::Cli(flags) => cli::run_cli(flags).await,
        RunMode::Server(flags) => server::run_server(flags).await,
    }
}
