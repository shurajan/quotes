use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use ticker_loader::load_tickers;

mod ticker_loader;

#[derive(Parser, Debug)]
#[command(
    name = "qserver",
    version = "0.1.0",
    about = "Streams quotes for a set of tickers",
    long_about = None
)]
struct Args {
    /// Optional path to tickers file
    #[arg(short, long, value_name = "FILE")]
    tickers: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let tickers = load_tickers(args.tickers)
        .context("failed to load tickers")?;

    println!("Loaded {} tickers", tickers.len());

    for ticker in tickers {
        println!("{ticker}");
    }

    Ok(())
}