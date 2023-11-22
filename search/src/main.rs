use clap::Parser;

mod config;
mod logger;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init("searxiv.log")?;

    let _cfg = config::Config::parse();

    Ok(())
}
