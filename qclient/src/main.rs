use clap::Parser;
use log::{error, info, warn};
use qlib::stock_quote::StockQuote;
use std::fs;
use std::io::Write;
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(name = "stock-receiver", about = "UDP stock quote receiver")]
struct Args {
    /// TCP server address and port
    #[arg(short = 's', long, default_value = "127.0.0.1:3000")]
    server: String,

    /// Local UDP port for receiving data
    #[arg(short = 'p', long, default_value_t = 34254)]
    udp_port: u16,

    /// Path to file with ticker list (one per line)
    #[arg(short = 't', long)]
    tickers_file: String,
}

fn main() {
    env_logger::init();

    if let Err(e) = run() {
        error!("{}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let tickers = read_tickers(&args.tickers_file);
    if tickers.is_empty() {
        return Err(format!("No tickers found in {}", args.tickers_file).into());
    }

    let udp_addr = format!("127.0.0.1:{}", args.udp_port);
    let stock_refs: Vec<&str> = tickers.iter().map(|s| s.as_str()).collect();

    let shutdown = Arc::new(AtomicBool::new(false));

    let shutdown_hook = shutdown.clone();
    ctrlc::set_handler(move || {
        info!("Ctrl+C received, shutting down...");
        shutdown_hook.store(true, Ordering::Relaxed);
    })?;

    let socket = UdpSocket::bind(&udp_addr)?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    info!("Receiver bound on {}", udp_addr);

    send_stream_command(&args.server, &udp_addr, &stock_refs)?;

    info!("Waiting for data...");
    receive_loop(socket, shutdown)?;

    info!("Receiver stopped");
    Ok(())
}

fn read_tickers(path: &str) -> Vec<String> {
    match fs::read_to_string(path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect(),
        Err(e) => {
            error!("Failed to read tickers file '{}': {}", path, e);
            Vec::new()
        }
    }
}

fn send_stream_command(
    server_addr: &str,
    udp_addr: &str,
    stocks: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let tickers = stocks.join(",");
    let command = format!("STREAM udp://{} {}\n", udp_addr, tickers);

    info!("Connecting to server at {}", server_addr);
    let mut stream = TcpStream::connect(server_addr)?;
    stream.write_all(command.as_bytes())?;
    stream.flush()?;
    info!("Sent command: {}", command.trim());

    Ok(())
}

fn receive_loop(
    socket: UdpSocket,
    shutdown: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = [0u8; 1024];
    let mut is_pinger_active = false;
    let socket = Arc::new(socket);

    loop {
        if shutdown.load(Ordering::Relaxed) {
            warn!("Shutdown signal received, exiting receive loop");
            return Ok(());
        }

        match socket.recv_from(&mut buf) {
            Ok((size, src_addr)) => {
                if let Some(stock_quote) = StockQuote::from_bytes(&buf[..size]) {
                    if !is_pinger_active {
                        is_pinger_active = true;
                        let ping_socket = socket.clone();
                        let ping_shutdown = shutdown.clone();
                        thread::spawn(move || {
                            ping(ping_socket, src_addr, ping_shutdown);
                        });
                    }
                    println!("{}\n{}", src_addr, stock_quote);
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(e) => {
                return Err(Box::new(e));
            }
        }
    }
}

fn ping(socket: Arc<UdpSocket>, src_addr: SocketAddr, shutdown: Arc<AtomicBool>) {
    const MAX_FAILURES: u32 = 3;
    let mut failures: u32 = 0;

    while !shutdown.load(Ordering::Relaxed) {
        match socket.send_to(b"PING", &src_addr) {
            Ok(_) => {
                failures = 0;
            }
            Err(e) => {
                failures += 1;
                error!("Ping failed ({}/{}): {}", failures, MAX_FAILURES, e);
                if failures >= MAX_FAILURES {
                    error!(
                        "Max ping failures reached ({}), shutting down",
                        MAX_FAILURES
                    );
                    shutdown.store(true, Ordering::Relaxed);
                    return;
                }
            }
        }
        for _ in 0..20 {
            if shutdown.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}
