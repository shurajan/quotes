# quotes

A real-time stock price streaming simulation system built in Rust. 
The server generates simulated price data for configurable stock tickers and streams it to clients over UDP, 
with connection management over TCP.

## Architecture

The project is a Cargo workspace with three crates:

- **`qlib`** — Shared library: `StockQuote` data model, binary/text serialization, and ticker file loading.
- **`qserver`** — Server binary: generates simulated prices and streams them to connected clients.
- **`qclient`** — Client binary: registers with the server and displays incoming quotes.

### Data Flow

```
qserver
  ├── Price Generator (every 150ms) ──→ EventBus (pub-sub)
  └── TCP Listener
        └── Per-Client Handler
              ├── Pinger Thread   (UDP recv: PING from client)
              ├── Sender Thread   (UDP send: 28-byte quote packets)
              └── Watchdog Thread (disconnects on ping timeout)

qclient
  ├── TCP: send STREAM command with UDP address + ticker list
  ├── UDP: receive 28-byte StockQuote packets, display quotes
  └── Pinger Thread: send PING every 2 seconds to stay alive
```

## Wire Format

Each quote is serialized as a 28-byte big-endian binary packet:

| Bytes | Field     | Type        |
|-------|-----------|-------------|
| 0–7   | Ticker    | UTF-8, null-padded |
| 8–15  | Price     | f64         |
| 16–19 | Volume    | u32         |
| 20–27 | Timestamp | u64 (Unix seconds) |

## Usage

### Build

```sh
cargo build --release
```

### Run the Server

```sh
# Default: all tickers, 127.0.0.1:3000, 150ms update interval
./target/release/qserver

# Custom options
./target/release/qserver \
  --tickers config/five_tickers.txt \
  --addr 127.0.0.1:3000 \
  --interval-ms 100 \
  --max-pct 1.0 \
  --log-level debug
```

**Server options:**

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--tickers` | `-t` | (embedded default) | Path to ticker symbols file |
| `--addr` | `-a` | `127.0.0.1:3000` | TCP listen address |
| `--interval-ms` | `-i` | `150` | Price update interval (ms) |
| `--max-pct` | `-m` | `0.5` | Max price change per tick (%) |
| `--log-level` | `-l` | `info` | Log level |

### Run the Client

```sh
./target/release/qclient \
  --server 127.0.0.1:3000 \
  --udp-port 34254 \
  --tickers-file config/five_tickers.txt
```

**Client options:**

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--server` | `-s` | `127.0.0.1:3000` | TCP server address |
| `--udp-port` | `-p` | `34254` | Local UDP port to receive quotes on |
| `--tickers-file` | `-t` | (required) | Path to file listing desired tickers |

### Ticker Files

Ticker files contain one symbol per line (case-insensitive, deduplicated):

```
AAPL
MSFT
GOOGL
```
