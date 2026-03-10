use qlib::stock_quote::StockQuote;
use rand::{RngExt, random_range};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct Prices {
    prices: Vec<StockQuote>,
    last_reset_ts: u64,
}

impl Prices {
    const RESET_INTERVAL: u64 = 3600;

    pub fn new(tickers: Vec<String>) -> Self {
        let now = Self::now_ts();
        let mut prices = Vec::new();
        tickers.iter().for_each(|ticker| {
            prices.push(StockQuote {
                ticker: ticker.clone(),
                price: random_range(1.0..5000.0),
                volume: 0,
                timestamp: now,
            })
        });

        Self {
            prices,
            last_reset_ts: now,
        }
    }

    pub fn update_prices(&mut self, max_pct: f64) {
        let mut rng = rand::rng();
        let ts = Self::now_ts();

        if ts - self.last_reset_ts >= Self::RESET_INTERVAL {
            for quote in self.prices.iter_mut() {
                quote.volume = 0;
            }
            self.last_reset_ts = ts;
        }

        for quote in self.prices.iter_mut() {
            let change = rng.random_range(-max_pct..=max_pct) / 100.0;
            quote.price = (quote.price * (1.0 + change)).max(0.01);
            quote.volume += rng.random_range(1..=3);
            quote.timestamp = ts;
        }
    }

    pub fn quotes(&self) -> &[StockQuote] {
        &self.prices
    }

    fn now_ts() -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_all_tickers() {
        let p = Prices::new(vec!["AAPL".into(), "GOOG".into()]);
        assert_eq!(p.prices.len(), 2);
        assert_eq!(p.prices[0].ticker, "AAPL");
        assert_eq!(p.prices[1].ticker, "GOOG");
    }

    #[test]
    fn new_volume_starts_at_zero() {
        let p = Prices::new(vec!["X".into()]);
        assert_eq!(p.prices[0].volume, 0);
    }

    #[test]
    fn update_changes_price_and_volume() {
        let mut p = Prices::new(vec!["X".into()]);
        let price_before = p.prices[0].price;
        p.update_prices(5.0);
        assert_ne!(p.prices[0].price, price_before);
        assert!(p.prices[0].volume > 0);
    }

    #[test]
    fn price_never_below_minimum() {
        let mut p = Prices::new(vec!["X".into()]);
        p.prices[0].price = 0.01;
        for _ in 0..500 {
            p.update_prices(50.0);
        }
        assert!(p.prices[0].price >= 0.01);
    }

    #[test]
    fn volume_resets_after_interval() {
        let mut p = Prices::new(vec!["X".into()]);
        p.update_prices(5.0);
        assert!(p.prices[0].volume > 0);

        p.last_reset_ts -= Prices::RESET_INTERVAL;
        p.update_prices(5.0);
        assert!(p.prices[0].volume <= 3);
    }
}
