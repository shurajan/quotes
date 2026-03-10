use std::fmt::Display;

/// Ticker
pub type Ticker = String;

/// Ticker size in binary format (bytes). Padded with `\0` on the right.
pub const TICKER_SIZE: usize = 8;

/// Full binary packet size (bytes).
///
/// | Field     | Offset   | Size   | Format          |
/// |-----------|----------|--------|-----------------|
/// | ticker    | 0        | 8      | UTF-8 + `\0`-pad|
/// | price     | 8        | 8      | f64 BE          |
/// | volume    | 16       | 4      | u32 BE          |
/// | timestamp | 20       | 8      | u64 BE          |
pub const PACKET_SIZE: usize = TICKER_SIZE + 8 + 4 + 8; // 28

/// Stock quote — a snapshot of price, volume, and time for a single ticker.
///
/// Supports two serialization formats:
/// - **binary** ([`to_bytes`] / [`from_bytes`]) — 28 bytes, big-endian,
///   intended for UDP transmission;
/// - **text** ([`serialize`] / [`deserialize`]) — `|`-delimited,
///   convenient for logs and debugging.
///
/// # Example
///
/// ```
/// use qlib::stock_quote::StockQuote;
///
/// let q = StockQuote {
///     ticker: "AAPL".into(),
///     price: 189.50,
///     volume: 1_200_000,
///     timestamp: 1_700_000_000,
/// };
///
/// // Binary roundtrip
/// let bytes = q.to_bytes();
/// assert_eq!(bytes.len(), 28);
/// let restored = StockQuote::from_bytes(&bytes).unwrap();
/// assert_eq!(restored.ticker, "AAPL");
/// ```
#[derive(Debug, Clone)]
pub struct StockQuote {
    /// Stock ticker, e.g. `"AAPL"`, `"TSLA"`.
    /// No more than [`TICKER_SIZE`] bytes in UTF-8.
    pub ticker: Ticker,
    /// Price per share (USD). Cannot be negative in valid data.
    pub price: f64,
    /// Trading volume (shares). `u32` limits max to ~4.29 billion.
    pub volume: u32,
    /// Unix time (seconds) of the quote moment.
    pub timestamp: u64,
}

/// Human-readable output with column alignment.
///
/// ```text
/// AAPL:
///   prc:     189.50
///   vol:    1200000
///   tme: 1700000000
/// ```
impl Display for StockQuote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:\n  prc: {:>10.2}\n  vol: {:>10}\n  tme: {}",
            self.ticker, self.price, self.volume, self.timestamp
        )
    }
}

// ── Binary serialization (for UDP) ────────────────

impl StockQuote {
    /// Serializes the quote into a fixed [`PACKET_SIZE`]-byte buffer (big-endian).
    pub fn to_bytes(&self) -> [u8; PACKET_SIZE] {
        let mut buf = [0u8; PACKET_SIZE];

        // ticker → first TICKER_SIZE bytes, remainder filled with \0
        let ticker_bytes = self.ticker.as_bytes();
        let len = ticker_bytes.len().min(TICKER_SIZE);
        buf[..len].copy_from_slice(&ticker_bytes[..len]);
        buf[8..16].copy_from_slice(&self.price.to_be_bytes());
        buf[16..20].copy_from_slice(&self.volume.to_be_bytes());
        buf[20..28].copy_from_slice(&self.timestamp.to_be_bytes());

        buf
    }

    /// Restores a quote from a [`PACKET_SIZE`]-byte buffer.
    /// Returns `None` if:
    /// - the slice length ≠ [`PACKET_SIZE`],
    /// - the ticker field contains invalid UTF-8.
    pub fn from_bytes(buf: &[u8]) -> Option<Self> {
        if buf.len() != PACKET_SIZE {
            return None;
        }

        // ticker: trim trailing \0
        let ticker_raw = &buf[..TICKER_SIZE];
        let ticker_end = ticker_raw
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(TICKER_SIZE);
        let ticker = std::str::from_utf8(&ticker_raw[..ticker_end])
            .ok()?
            .to_string();

        let price = f64::from_be_bytes(buf[8..16].try_into().ok()?);
        let volume = u32::from_be_bytes(buf[16..20].try_into().ok()?);
        let timestamp = u64::from_be_bytes(buf[20..28].try_into().ok()?);

        Some(StockQuote {
            ticker,
            price,
            volume,
            timestamp,
        })
    }
}

impl StockQuote {
    /// Serializes the quote into a `|`-delimited string.
    pub fn serialize(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.ticker, self.price, self.volume, self.timestamp
        )
    }

    /// Restores a quote from a string in `TICKER|PRICE|VOLUME|TIMESTAMP` format.
    pub fn deserialize(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('|').collect();
        if parts.len() == 4 {
            Some(StockQuote {
                ticker: parts[0].to_string(),
                price: parts[1].parse().ok()?,
                volume: parts[2].parse().ok()?,
                timestamp: parts[3].parse().ok()?,
            })
        } else {
            None
        }
    }
}

// ─────────────────────────────────────────────
//  Tests
// ─────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StockQuote {
        StockQuote {
            ticker: "AAPL".into(),
            price: 189.50,
            volume: 1_200_000,
            timestamp: 1_700_000_000,
        }
    }
    #[test]
    fn binary_packet_size() {
        assert_eq!(sample().to_bytes().len(), PACKET_SIZE);
        assert_eq!(PACKET_SIZE, 28);
    }

    #[test]
    fn binary_roundtrip() {
        let original = sample();
        let restored = StockQuote::from_bytes(&original.to_bytes()).unwrap();

        assert_eq!(original.ticker, restored.ticker);
        assert!((original.price - restored.price).abs() < f64::EPSILON);
        assert_eq!(original.volume, restored.volume);
        assert_eq!(original.timestamp, restored.timestamp);
    }

    #[test]
    fn binary_ticker_padded_with_zeros() {
        let buf = sample().to_bytes();
        // "AAPL" = 4 bytes, remaining 4 should be \0
        assert_eq!(&buf[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn binary_ticker_exact_max_length() {
        let q = StockQuote {
            ticker: "ABCDEFGH".into(), // 8 bytes
            ..sample()
        };
        let restored = StockQuote::from_bytes(&q.to_bytes()).unwrap();
        assert_eq!(restored.ticker, "ABCDEFGH");
    }

    #[test]
    fn binary_ticker_truncated_if_too_long() {
        let q = StockQuote {
            ticker: "LONGTICKERXYZ".into(),
            ..sample()
        };
        let restored = StockQuote::from_bytes(&q.to_bytes()).unwrap();
        assert_eq!(restored.ticker, "LONGTICK");
    }

    #[test]
    fn binary_empty_ticker() {
        let q = StockQuote {
            ticker: "".into(),
            ..sample()
        };
        let restored = StockQuote::from_bytes(&q.to_bytes()).unwrap();
        assert_eq!(restored.ticker, "");
    }

    #[test]
    fn binary_big_endian_price() {
        let q = StockQuote {
            price: 1.0,
            ..sample()
        };
        let buf = q.to_bytes();
        assert_eq!(&buf[8..16], &0x3FF0_0000_0000_0000u64.to_be_bytes());
    }

    #[test]
    fn binary_big_endian_volume() {
        let q = StockQuote {
            volume: 0x01020304,
            ..sample()
        };
        let buf = q.to_bytes();
        assert_eq!(&buf[16..20], &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn binary_big_endian_timestamp() {
        let q = StockQuote {
            timestamp: 1,
            ..sample()
        };
        let buf = q.to_bytes();
        assert_eq!(&buf[20..28], &[0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn binary_zero_values() {
        let q = StockQuote {
            ticker: "".into(),
            price: 0.0,
            volume: 0,
            timestamp: 0,
        };
        let buf = q.to_bytes();
        assert_eq!(buf, [0u8; PACKET_SIZE]);
    }

    #[test]
    fn binary_max_volume_and_timestamp() {
        let q = StockQuote {
            volume: u32::MAX,
            timestamp: u64::MAX,
            ..sample()
        };
        let restored = StockQuote::from_bytes(&q.to_bytes()).unwrap();
        assert_eq!(restored.volume, u32::MAX);
        assert_eq!(restored.timestamp, u64::MAX);
    }

    #[test]
    fn from_bytes_wrong_size_short() {
        assert!(StockQuote::from_bytes(&[0u8; 10]).is_none());
    }

    #[test]
    fn from_bytes_wrong_size_long() {
        assert!(StockQuote::from_bytes(&[0u8; 30]).is_none());
    }

    #[test]
    fn from_bytes_empty() {
        assert!(StockQuote::from_bytes(&[]).is_none());
    }

    #[test]
    fn from_bytes_invalid_utf8_ticker() {
        let mut buf = sample().to_bytes();
        buf[0] = 0xFF;
        buf[1] = 0xFE;
        assert!(StockQuote::from_bytes(&buf).is_none());
    }
    #[test]
    fn text_serialize_basic() {
        assert_eq!(sample().serialize(), "AAPL|189.5|1200000|1700000000");
    }

    #[test]
    fn text_deserialize_basic() {
        let q = StockQuote::deserialize("TSLA|242.7|800000|1700000000").unwrap();
        assert_eq!(q.ticker, "TSLA");
        assert!((q.price - 242.7).abs() < f64::EPSILON);
    }

    #[test]
    fn text_roundtrip() {
        let original = sample();
        let restored = StockQuote::deserialize(&original.serialize()).unwrap();
        assert_eq!(original.ticker, restored.ticker);
        assert!((original.price - restored.price).abs() < f64::EPSILON);
        assert_eq!(original.volume, restored.volume);
        assert_eq!(original.timestamp, restored.timestamp);
    }

    #[test]
    fn text_deserialize_empty() {
        assert!(StockQuote::deserialize("").is_none());
    }

    #[test]
    fn text_deserialize_too_few_fields() {
        assert!(StockQuote::deserialize("AAPL|100").is_none());
    }

    #[test]
    fn text_deserialize_too_many_fields() {
        assert!(StockQuote::deserialize("AAPL|100|200|300|extra").is_none());
    }

    #[test]
    fn text_deserialize_bad_price() {
        assert!(StockQuote::deserialize("AAPL|abc|200|300").is_none());
    }

    #[test]
    fn to_string_uses_display_not_serialize() {
        let q = sample();
        let display = q.to_string();
        let serial = q.serialize();
        assert!(display.contains("prc:"));
        assert!(!serial.contains("prc:"));
    }

    #[test]
    fn display_price_two_decimals() {
        assert!(format!("{}", sample()).contains("189.50"));
    }
}
