use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::{Rng, RngExt};
use qlib::stock_quote::StockQuote;

struct Prices {
    prices: HashMap<String, f64>,
}
impl Prices {
    pub fn generate_quote(&mut self, ticker: &str) -> Option<StockQuote> {
        fn next_price(current: f64, max_pct: f64, rng: &mut impl Rng) -> f64 {
            let change = rng.random_range(-max_pct..=max_pct) / 100.0;
            (current * (1.0 + change)).max(0.01)
        }

        let price = self.prices.get(ticker).copied().unwrap_or_else(|| rand::random::<f64>());
        let mut rng = rand::rng();
        let new_price = next_price(price, 3.5, &mut rng);
        self.prices.insert(ticker.to_owned(), new_price);

        let volume = match ticker {
            // Популярные акции имеют больший объём
            "AAPL" | "MSFT" | "TSLA" => 1000 + (rand::random::<f64>() * 5000.0) as u32,
            // Обычные акции - средний объём
            _ => 100 + (rand::random::<f64>() * 1000.0) as u32,
        };

        let new_price = next_price(volume as f64, 3.5, &mut rng);
        Some(StockQuote {
            ticker: ticker.to_string(),
            price: new_price,
            volume,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64,
        })
    }
}