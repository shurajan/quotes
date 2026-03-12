use crate::market_data::Event;
use anyhow::Context;
use clap::Parser;
use market_data::EventBus;
use prices::Prices;
use qlib::ticker_loader::load_tickers;
use std::io::{BufRead, BufReader};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

mod market_data;
mod prices;

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
    #[arg(short, long, default_value = "150")]
    interval_ms: u64,
    /// Max price change per tick in percent
    #[arg(short, long, default_value = "0.5")]
    max_pct: f64,
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    addr: String,
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

    let bus = Arc::new(EventBus::new());
    let interval_ms = args.interval_ms;
    let max_pct = args.max_pct;

    log::info!("Starting tcp listener: {}", args.addr);
    let listener_bus = Arc::clone(&bus);
    thread::spawn(move || {
        if let Err(e) = tcp_listener(args.addr, listener_bus) {
            log::error!("TCP listener failed: {e}");
        }
    });

    let mut prices = Prices::new(tickers.clone());

    let rx = bus.subscribe(&tickers)?;
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

    let interval = Duration::from_millis(interval_ms);
    log::info!(
        "Starting price feed: {}ms interval, {:.1}% max change",
        interval_ms,
        max_pct
    );

    loop {
        prices.update_prices(max_pct);

        for quote in prices.quotes() {
            bus.publish(Event {
                ticker: quote.ticker.clone(),
                quote: quote.clone(),
            })?;
        }

        thread::sleep(interval);
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    pub ip: String,
    pub port: u16,
    pub stocks: Vec<String>,
}

fn tcp_listener(addr: String, bus: Arc<EventBus>) -> std::io::Result<()> {
    let listener = TcpListener::bind(&addr)?;

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let client_bus = Arc::clone(&bus);
                thread::spawn(move || {
                    if let Err(e) = handle_client(stream, client_bus) {
                        log::error!("Client error: {e}");
                    }
                });
            }
            Err(e) => log::error!("Accept error: {e}"),
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, bus: Arc<EventBus>) -> anyhow::Result<()> {
    let peer = stream.peer_addr()?;
    log::info!("New connection from {peer}");

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let line = line.trim();

    let client =
        parse_command(line).with_context(|| format!("invalid command from {peer}: {line}"))?;

    log::info!(
        "Client {peer} → udp://{}:{} stocks={:?}",
        client.ip,
        client.port,
        client.stocks
    );

    let rx = bus.subscribe(&client.stocks)?;
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let dest = format!("{}:{}", client.ip, client.port);

    // поток для чтения PING
    let ping_socket = socket.try_clone()?;
    thread::spawn(move || {
        let mut buf = [0u8; 64];
        loop {
            match ping_socket.recv_from(&mut buf) {
                Ok((size, addr)) => {
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    log::info!("Received {} from {}", msg, addr);
                }
                Err(e) => {
                    log::error!("Can not read PING: {}", e);
                    break;
                }
            }
        }
    });

    for event in rx {
        let bytes = event.quote.to_bytes();
        if let Err(e) = socket.send_to(&bytes, &dest) {
            log::error!("UDP send to {dest} failed: {e}");
            break;
        }
    }

    log::info!("Client {peer} disconnected");
    Ok(())
}

fn parse_command(line: &str) -> anyhow::Result<Client> {
    let parts: Vec<&str> = line.splitn(3, ' ').collect();
    anyhow::ensure!(
        parts.len() == 3 && parts[0] == "STREAM",
        "expected: STREAM udp://ip:port TICKER,..."
    );

    let url = parts[1]
        .strip_prefix("udp://")
        .context("expected udp:// prefix")?;

    let (ip, port_str) = url.rsplit_once(':').context("missing :port")?;
    let port: u16 = port_str.parse().context("invalid port")?;

    let stocks = parts[2]
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(Client {
        ip: ip.to_string(),
        port,
        stocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_command() {
        let client = parse_command("STREAM udp://127.0.0.1:34254 AAPL,TSLA").unwrap();
        assert_eq!(client.ip, "127.0.0.1");
        assert_eq!(client.port, 34254);
        assert_eq!(client.stocks, vec!["AAPL", "TSLA"]);
    }

    #[test]
    fn parse_single_ticker() {
        let client = parse_command("STREAM udp://10.0.0.1:5000 MSFT").unwrap();
        assert_eq!(client.stocks, vec!["MSFT"]);
    }

    #[test]
    fn parse_rejects_missing_prefix() {
        assert!(parse_command("STREAM tcp://127.0.0.1:3000 AAPL").is_err());
    }

    #[test]
    fn parse_rejects_bad_command() {
        assert!(parse_command("SUBSCRIBE udp://127.0.0.1:3000 AAPL").is_err());
    }

    #[test]
    fn parse_rejects_bad_port() {
        assert!(parse_command("STREAM udp://127.0.0.1:99999 AAPL").is_err());
    }

    #[test]
    fn parse_rejects_empty() {
        assert!(parse_command("").is_err());
    }
}
