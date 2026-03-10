use crate::market_data::Event;
use anyhow::Context;
use clap::Parser;
use market_data::EventBus;
use prices::Prices;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use ticker_loader::load_tickers;

mod market_data;
mod prices;
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
    /// Logging level: error, warn, info, debug, trace
    #[arg(short, long, default_value = "info")]
    log_level: String,
    /// Price update interval in milliseconds
    #[arg(short, long, default_value = "100")]
    interval_ms: u64,
    /// Max price change per tick in percent
    #[arg(short, long, default_value = "2.0")]
    max_pct: f64,
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::new()
        .parse_filters(&args.log_level)
        .init();

    if let Err(err) = run(args) {
        log::error!("{err:#}");
    }
}

fn run(args: Args) -> anyhow::Result<()> {
    let path_display = args.tickers.as_ref().map(|p| p.display().to_string());
    let tickers = load_tickers(args.tickers).with_context(|| match path_display {
        Some(p) => format!("failed to load tickers from {p}"),
        None => "failed to load default tickers".into(),
    })?;

    log::info!("Loaded {} tickers", tickers.len());
    for ticker in &tickers {
        log::debug!("{ticker}");
    }

    let bus = EventBus::new();
    let mut prices = Prices::new(tickers.clone());

    let rx = bus.subscribe(&tickers);
    thread::spawn(move || {
        for event in rx {
            log::debug!(
                "{}: price={:.2} vol={}",
                event.ticker,
                event.quote.price,
                event.quote.volume
            );
        }
    });

    //TODO - delete
    let rx_test = bus.subscribe(&*vec!["AAPL".to_string(), "MSFT".to_string()]);
    thread::spawn(move || {
        for event in rx_test {
            log::info!(
                "{}: price={:.2} vol={}",
                event.ticker,
                event.quote.price,
                event.quote.volume
            );
        }
    });

    let interval = Duration::from_millis(args.interval_ms);
    log::info!(
        "Starting price feed: {}ms interval, {:.1}% max change",
        args.interval_ms,
        args.max_pct
    );

    loop {
        prices.update_prices(args.max_pct);

        for quote in prices.quotes() {
            bus.publish(Event {
                ticker: quote.ticker.clone(),
                quote: quote.clone(),
            });
        }

        thread::sleep(interval);
    }
}
