use crossbeam_channel::{Receiver, Sender, unbounded};
use qlib::stock_quote::{StockQuote, Ticker};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Event {
    pub(crate) ticker: Ticker,
    pub(crate) quote: StockQuote,
}

#[derive(Clone)]
pub struct EventBus {
    subs: Arc<Mutex<HashMap<Ticker, Vec<Sender<Event>>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Subscription to the list of stocks.
    /// Returns one Receiver to gather all events.
    pub fn subscribe(&self, tickers: &[Ticker]) -> Receiver<Event> {
        let (tx, rx) = unbounded();
        let mut subs = self.subs.lock().unwrap();
        for id in tickers {
            subs.entry(id.to_string()).or_default().push(tx.clone());
        }
        rx
    }

    pub fn publish(&self, event: Event) {
        let mut subs = self.subs.lock().unwrap();
        if let Some(senders) = subs.get_mut(&event.ticker) {
            senders.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscribe_and_receive() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into()]);

        let event = Event {
            ticker: "AAPL".into(),
            quote: StockQuote {
                ticker: "AAPL".into(),
                price: 150.0,
                volume: 10,
                timestamp: 0,
            },
        };

        bus.publish(event.clone());
        let received = rx.try_recv().unwrap();
        assert_eq!(received.ticker, "AAPL");
        assert_eq!(received.quote.price, 150.0);
    }

    #[test]
    fn no_event_for_other_ticker() {
        let bus = EventBus::new();
        let rx = bus.subscribe(&["AAPL".into()]);

        bus.publish(Event {
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
        let rx = bus.subscribe(&["AAPL".into(), "GOOG".into()]);

        for ticker in &["AAPL", "GOOG"] {
            bus.publish(Event {
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
        let rx = bus.subscribe(&["AAPL".into()]);
        drop(rx);

        bus.publish(Event {
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
