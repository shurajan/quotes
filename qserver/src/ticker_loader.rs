use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_TICKERS: &str = include_str!("../../config/tickers.txt");

#[derive(Debug)]
pub enum TickerLoadError {
    Io(std::io::Error),
    EmptyOrInvalidFile(String),
}

impl std::fmt::Display for TickerLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read tickers file: {err}"),
            Self::EmptyOrInvalidFile(path) => {
                write!(
                    f,
                    "tickers file '{path}' is empty or contains no valid tickers"
                )
            }
        }
    }
}

impl std::error::Error for TickerLoadError {}

pub fn load_tickers(path: Option<PathBuf>) -> Result<Vec<String>, TickerLoadError> {
    match path {
        Some(path) => load_tickers_from_file(path.as_path()),
        None => Ok(parse_tickers(DEFAULT_TICKERS)),
    }
}

fn load_tickers_from_file(path: &Path) -> Result<Vec<String>, TickerLoadError> {
    let content = fs::read_to_string(path).map_err(TickerLoadError::Io)?;
    let tickers = parse_tickers(&content);

    if tickers.is_empty() {
        return Err(TickerLoadError::EmptyOrInvalidFile(
            path.display().to_string(),
        ));
    }

    Ok(tickers)
}

fn parse_tickers(input: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for line in input.lines() {
        let ticker = normalize_ticker(line);

        if !is_valid_ticker(&ticker) {
            continue;
        }

        if seen.insert(ticker.clone()) {
            result.push(ticker);
        }
    }

    result
}

fn normalize_ticker(line: &str) -> String {
    line.trim().to_uppercase()
}

fn is_valid_ticker(ticker: &str) -> bool {
    !ticker.is_empty()
        && ticker
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_tickers_removes_duplicates_and_preserves_order() {
        let input = "\
AAPL
MSFT
AAPL
GOOGL
MSFT
NVDA
";

        let tickers = parse_tickers(input);

        assert_eq!(tickers, vec!["AAPL", "MSFT", "GOOGL", "NVDA"]);
    }

    #[test]
    fn parse_tickers_trims_and_uppercases() {
        let input = "\
 aapl
 msft
Googl
";

        let tickers = parse_tickers(input);

        assert_eq!(tickers, vec!["AAPL", "MSFT", "GOOGL"]);
    }

    #[test]
    fn parse_tickers_skips_empty_lines() {
        let input = "\
AAPL

MSFT


NVDA
";

        let tickers = parse_tickers(input);

        assert_eq!(tickers, vec!["AAPL", "MSFT", "NVDA"]);
    }

    #[test]
    fn parse_tickers_skips_invalid_lines() {
        let input = "\
AAPL
MS FT
$$$
NVDA
BRK.B
RDS-A
";

        let tickers = parse_tickers(input);

        assert_eq!(tickers, vec!["AAPL", "NVDA", "BRK.B", "RDS-A"]);
    }

    #[test]
    fn load_tickers_uses_file_when_path_is_valid() {
        let path = make_temp_file_path("tickers_valid");
        fs::write(&path, "AAPL\nMSFT\nAAPL\n").unwrap();

        let tickers = load_tickers(Some(path.clone())).unwrap();

        assert_eq!(tickers, vec!["AAPL", "MSFT"]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_tickers_returns_error_when_file_does_not_exist() {
        let result = load_tickers(Some(PathBuf::from("this/file/should/not/exist.txt")));

        assert!(matches!(result, Err(TickerLoadError::Io(_))));
    }

    #[test]
    fn load_tickers_returns_error_when_file_is_empty() {
        let path = make_temp_file_path("tickers_empty");
        fs::write(&path, "").unwrap();

        let result = load_tickers(Some(path.clone()));

        assert!(matches!(
            result,
            Err(TickerLoadError::EmptyOrInvalidFile(_))
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_tickers_returns_error_when_file_has_only_invalid_lines() {
        let path = make_temp_file_path("tickers_invalid");
        fs::write(&path, "@@@\n   \n###\n").unwrap();

        let result = load_tickers(Some(path.clone()));

        assert!(matches!(
            result,
            Err(TickerLoadError::EmptyOrInvalidFile(_))
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn load_tickers_uses_defaults_when_path_is_none() {
        let tickers = load_tickers(None).unwrap();
        let defaults = parse_tickers(DEFAULT_TICKERS);

        assert_eq!(tickers, defaults);
    }

    fn make_temp_file_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        std::env::temp_dir().join(format!("{prefix}_{nanos}.txt"))
    }
}
