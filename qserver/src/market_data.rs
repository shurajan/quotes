use crossbeam_channel::{Receiver, Sender, unbounded};
use qlib::stock_quote::{StockQuote, Ticker};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Clone, Debug)]
pub(crate) struct Event {
    pub(crate) ticker: Ticker,
    pub(crate) quote: StockQuote,
}

#[derive(Clone)]
pub struct EventBus {
    subs: Arc<Mutex<HashMap<Ticker, Vec<Sender<Event>>>>>,
}

#[derive(Error, Debug)]
pub enum BusError {
    #[error("lock poisoned")]
    LockPoisoned,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self, tickers: &[Ticker]) -> Result<Receiver<Event>, BusError> {
        let (tx, rx) = unbounded();
        let mut subs = self.subs.lock().map_err(|_| BusError::LockPoisoned)?;
        for id in tickers {
            subs.entry(id.to_string()).or_default().push(tx.clone());
        }
        Ok(rx)
    }

    pub fn publish(&self, event: Event) -> Result<(), BusError> {
        let mut subs = self.subs.lock().map_err(|_| BusError::LockPoisoned)?;
        if let Some(senders) = subs.get_mut(&event.ticker) {
            senders.retain(|tx| tx.send(event.clone()).is_ok());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_and_receive() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into()]).unwrap();

        let event = Event {
            ticker: "AAPL".into(),
            quote: StockQuote {
                ticker: "AAPL".into(),
                price: 150.0,
                volume: 10,
                timestamp: 0,
            },
        };

        let _ = bus.publish(event.clone());
        let received = rx.try_recv().unwrap();
        assert_eq!(received.ticker, "AAPL");
        assert_eq!(received.quote.price, 150.0);
    }

    #[test]
    fn no_event_for_other_ticker() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into()]).unwrap();

        let _ = bus.publish(Event {
            ticker: "GOOG".into(),
            quote: StockQuote {
                ticker: "GOOG".into(),
                price: 100.0,
                volume: 0,
                timestamp: 0,
            },
        });

        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn multiple_tickers_one_receiver() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into(), "GOOG".into()]).unwrap();

        for ticker in &["AAPL", "GOOG"] {
            let _ = bus.publish(Event {
                ticker: ticker.to_string(),
                quote: StockQuote {
                    ticker: ticker.to_string(),
                    price: 1.0,
                    volume: 0,
                    timestamp: 0,
                },
            });
        }

        assert_eq!(rx.try_recv().unwrap().ticker, "AAPL");
        assert_eq!(rx.try_recv().unwrap().ticker, "GOOG");
    }

    #[test]
    fn dropped_receiver_cleaned_up() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into()]).unwrap();
        drop(rx);

        let _ = bus.publish(Event {
            ticker: "AAPL".into(),
            quote: StockQuote {
                ticker: "AAPL".into(),
                price: 1.0,
                volume: 0,
                timestamp: 0,
            },
        });

        let subs = bus.subs.lock().unwrap();
        assert!(subs.get("AAPL").unwrap().is_empty());
    }
}
