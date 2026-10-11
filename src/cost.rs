//! Tier prices from a user-supplied JSON file, and the monthly estimate.
//!
//! Shape: `{ "currency": "...", "tiers": [ { "name": "...", "price_per_rum_hour": n } ] }`.
//! A month is RUM × price per RUM-hour × hours (730 unless the caller passes
//! another value), rounded half away from zero to two decimal places.
//! A product that is not finite, or that does not fit in the cent amount,
//! is [`CostOverflow`]: the amount is never clamped.
//! The program has no built-in prices. Error text is the path and the kind of
//! problem, never bytes from the file.
//!
//! Only a regular file is read. [`std::fs::metadata`] and `is_file` run before
//! [`std::fs::read`]. A directory, a FIFO or a device is refused without
//! reading a byte.

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive};

/// Hours in the monthly estimate when `--hours-per-month` is absent.
pub const DEFAULT_HOURS_PER_MONTH: u64 = 730;

/// A price list read from disk. `price_per_rum_hour` is the number from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceList {
    pub currency: String,
    pub tiers: Vec<TierPrice>,
}

/// One tier. The name and the price are taken from the file as given.
#[derive(Debug, Clone, PartialEq)]
pub struct TierPrice {
    pub name: String,
    pub price_per_rum_hour: f64,
}

/// One tier of the monthly estimate. `amount` is already rounded to two decimals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierCost {
    pub name: String,
    pub currency: String,
    pub amount: String,
}

/// The monthly amount does not fit. Callers report this as an input error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostOverflow;

impl fmt::Display for CostOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("monthly cost overflows")
    }
}

impl std::error::Error for CostOverflow {}

/// Monthly cost of `rum` on every tier, in the price file's tier order.
pub fn monthly_costs(
    rum: u64,
    prices: &PriceList,
    hours_per_month: u64,
) -> Result<Vec<TierCost>, CostOverflow> {
    prices
        .tiers
        .iter()
        .map(|tier| {
            Ok(TierCost {
                name: tier.name.clone(),
                currency: prices.currency.clone(),
                amount: format_cents(monthly_cents(
                    rum,
                    tier.price_per_rum_hour,
                    hours_per_month,
                )?),
            })
        })
        .collect()
}

/// Rounded monthly amount in cents. `diff` subtracts these integers.
pub(crate) fn monthly_cents(
    rum: u64,
    price_per_rum_hour: f64,
    hours_per_month: u64,
) -> Result<u64, CostOverflow> {
    if !price_per_rum_hour.is_finite() || price_per_rum_hour < 0.0 {
        return Err(CostOverflow);
    }
    // Use the price's decimal representation so values such as 1.005 round
    // to 1.01. Binary float multiplication can put a half cent below the tie,
    // lose integer precision, or overflow before multiplying by zero hours.
    let price = BigDecimal::from_str(&price_per_rum_hour.to_string()).map_err(|_| CostOverflow)?;
    let scaled = BigDecimal::from(rum) * price * BigDecimal::from(hours_per_month) * 100u32;
    scaled
        .with_scale_round(0, RoundingMode::HalfUp)
        .to_u64()
        .ok_or(CostOverflow)
}

fn format_cents(cents: u64) -> String {
    format!("{}.{:02}", cents / 100, cents % 100)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PriceError {
    Unreadable {
        path: PathBuf,
        kind: &'static str,
    },
    /// The path exists and is not a regular file. Nothing was read.
    NotRegular {
        path: PathBuf,
    },
    InvalidJson {
        path: PathBuf,
        line: usize,
        column: usize,
    },
    Invalid {
        path: PathBuf,
    },
    NoTiers {
        path: PathBuf,
    },
    EmptyTierName {
        path: PathBuf,
    },
    TierNotObject {
        path: PathBuf,
    },
    NegativePrice {
        path: PathBuf,
    },
    NonNumericPrice {
        path: PathBuf,
    },
    EmptyCurrency {
        path: PathBuf,
    },
    /// UTF-16 or UTF-32. Nothing was decoded.
    NotUtf8 {
        path: PathBuf,
        encoding: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseError {
    InvalidJson { line: usize, column: usize },
    Invalid,
    NoTiers,
    EmptyTierName,
    TierNotObject,
    NegativePrice,
    NonNumericPrice,
    EmptyCurrency,
    NotUtf8(crate::encoding::Wide),
}

impl ParseError {
    fn with_path(self, path: &Path) -> PriceError {
        let path = path.to_path_buf();
        match self {
            Self::InvalidJson { line, column } => PriceError::InvalidJson { path, line, column },
            Self::Invalid => PriceError::Invalid { path },
            Self::NoTiers => PriceError::NoTiers { path },
            Self::EmptyTierName => PriceError::EmptyTierName { path },
            Self::TierNotObject => PriceError::TierNotObject { path },
            Self::NegativePrice => PriceError::NegativePrice { path },
            Self::NonNumericPrice => PriceError::NonNumericPrice { path },
            Self::EmptyCurrency => PriceError::EmptyCurrency { path },
            Self::NotUtf8(wide) => PriceError::NotUtf8 {
                path,
                encoding: wide.name(),
            },
        }
    }
}

impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { path, kind } => {
                write!(
                    f,
                    "rum-meter: {}: unreadable ({kind})",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NotRegular { path } => {
                write!(
                    f,
                    "rum-meter: {}: not a regular file",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::InvalidJson { path, line, column } => {
                write!(
                    f,
                    "rum-meter: {}: invalid json at line {line} column {column}",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::Invalid { path } => {
                write!(
                    f,
                    "rum-meter: {}: invalid price file",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NoTiers { path } => {
                write!(
                    f,
                    "rum-meter: {}: price file has no tiers",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::EmptyTierName { path } => {
                write!(
                    f,
                    "rum-meter: {}: tier name is empty",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::TierNotObject { path } => {
                write!(
                    f,
                    "rum-meter: {}: tier is not an object",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NegativePrice { path } => {
                write!(
                    f,
                    "rum-meter: {}: price is negative",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NonNumericPrice { path } => {
                write!(
                    f,
                    "rum-meter: {}: price is not a number",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::EmptyCurrency { path } => {
                write!(
                    f,
                    "rum-meter: {}: currency is empty",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::NotUtf8 { path, encoding } => {
                write!(
                    f,
                    "rum-meter: {}: file is {encoding}; save it as UTF-8",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
        }
    }
}

impl std::error::Error for PriceError {}

/// Read one price file. The error text never includes bytes from the file.
///
/// A path that is not a regular file is refused before [`fs::read`].
pub fn load_prices(path: &Path) -> Result<PriceList, PriceError> {
    reject_unless_regular_file(path)?;
    let bytes = fs::read(path).map_err(|err| PriceError::Unreadable {
        path: path.to_path_buf(),
        kind: io_kind_label(err.kind()),
    })?;
    parse_bytes(&bytes).map_err(|err| err.with_path(path))
}

/// One leading UTF-8 BOM is skipped; UTF-16 and UTF-32 are refused.
fn parse_bytes(bytes: &[u8]) -> Result<PriceList, ParseError> {
    let bytes = crate::encoding::utf8_body(bytes).map_err(ParseError::NotUtf8)?;
    let value = match serde_json::from_slice::<serde_json::Value>(bytes) {
        Ok(value) => value,
        Err(err) => {
            return Err(ParseError::InvalidJson {
                line: err.line(),
                column: err.column(),
            });
        }
    };
    let Some(obj) = value.as_object() else {
        return Err(ParseError::Invalid);
    };
    let currency = match obj.get("currency") {
        Some(serde_json::Value::String(text)) if !text.is_empty() => text.clone(),
        _ => return Err(ParseError::EmptyCurrency),
    };
    let Some(tiers) = obj.get("tiers").and_then(serde_json::Value::as_array) else {
        return Err(ParseError::NoTiers);
    };
    if tiers.is_empty() {
        return Err(ParseError::NoTiers);
    }
    let mut parsed = Vec::with_capacity(tiers.len());
    for tier in tiers {
        parsed.push(parse_tier(tier)?);
    }
    Ok(PriceList {
        currency,
        tiers: parsed,
    })
}

fn parse_tier(value: &serde_json::Value) -> Result<TierPrice, ParseError> {
    let Some(obj) = value.as_object() else {
        return Err(ParseError::TierNotObject);
    };
    let name = match obj.get("name") {
        Some(serde_json::Value::String(text)) if !text.is_empty() => text.clone(),
        _ => return Err(ParseError::EmptyTierName),
    };
    let price = match obj.get("price_per_rum_hour") {
        Some(value) => parse_price(value)?,
        None => return Err(ParseError::NonNumericPrice),
    };
    Ok(TierPrice {
        name,
        price_per_rum_hour: price,
    })
}

fn parse_price(value: &serde_json::Value) -> Result<f64, ParseError> {
    let Some(price) = value.as_f64() else {
        return Err(ParseError::NonNumericPrice);
    };
    if !price.is_finite() {
        return Err(ParseError::NonNumericPrice);
    }
    if price < 0.0 {
        return Err(ParseError::NegativePrice);
    }
    Ok(price)
}

/// `metadata().is_file()` before any read. A FIFO would block in `fs::read`
/// when nobody is writing, and `/dev/zero` would grow without a bound.
fn reject_unless_regular_file(path: &Path) -> Result<(), PriceError> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok(()),
        Ok(_) => Err(PriceError::NotRegular {
            path: path.to_path_buf(),
        }),
        Err(err) => Err(PriceError::Unreadable {
            path: path.to_path_buf(),
            kind: io_kind_label(err.kind()),
        }),
    }
}

fn io_kind_label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "not found",
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::IsADirectory => "is a directory",
        _ => "io error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CANARY: &str = "CANARY-SECRET-PR";

    fn write_price_file(name: &str, body: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("cost-prices");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(name);
        fs::write(&path, body).expect("write price file");
        path
    }

    fn load_body(name: &str, body: &str) -> Result<PriceList, PriceError> {
        assert!(!name.contains(CANARY), "the path would leak into the error");
        let path = write_price_file(name, body);
        let result = load_prices(&path);
        if let Err(err) = &result {
            let text = err.to_string();
            assert!(!text.contains(CANARY), "error text contains file content");
            assert_eq!(text.lines().count(), 1);
        }
        let _ = fs::remove_file(&path);
        result
    }

    #[test]
    fn valid_price_file_reads_currency_and_tiers() {
        let body = r#"{"currency":"EUR","note":"CANARY-SECRET-PR-VALID","tiers":[{"name":"small","price_per_rum_hour":1.25},{"name":"large","price_per_rum_hour":0}]}"#;
        let prices = load_body("valid.json", body).expect("valid price file");
        assert_eq!(prices.currency, "EUR");
        assert_eq!(prices.tiers.len(), 2);
        assert_eq!(prices.tiers[0].name, "small");
        assert_eq!(prices.tiers[0].price_per_rum_hour, 1.25);
        assert_eq!(prices.tiers[1].name, "large");
        assert_eq!(prices.tiers[1].price_per_rum_hour, 0.0);
        assert!(!format!("{prices:?}").contains(CANARY));
    }

    #[test]
    fn broken_json_is_invalid() {
        let err = load_body("broken.json", "CANARY-SECRET-PR-BROKEN {").unwrap_err();
        assert!(matches!(err, PriceError::InvalidJson { line: 1, .. }));
    }

    #[test]
    fn missing_or_empty_tiers_are_rejected() {
        let missing = load_body(
            "no-tiers.json",
            r#"{"currency":"EUR","note":"CANARY-SECRET-PR-NOTIERS"}"#,
        )
        .unwrap_err();
        assert!(matches!(missing, PriceError::NoTiers { .. }));

        let empty = load_body(
            "empty-tiers.json",
            r#"{"currency":"EUR","tiers":[],"note":"CANARY-SECRET-PR-EMPTYTIERS"}"#,
        )
        .unwrap_err();
        assert!(matches!(empty, PriceError::NoTiers { .. }));

        let not_array = load_body(
            "tiers-string.json",
            r#"{"currency":"EUR","tiers":"CANARY-SECRET-PR-TIERSTRING"}"#,
        )
        .unwrap_err();
        assert!(matches!(not_array, PriceError::NoTiers { .. }));
    }

    #[test]
    fn empty_tier_name_is_rejected() {
        let err = load_body(
            "empty-name.json",
            r#"{"currency":"EUR","tiers":[{"name":"","price_per_rum_hour":1,"note":"CANARY-SECRET-PR-EMPTYNAME"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, PriceError::EmptyTierName { .. }));
        assert!(err.to_string().ends_with("tier name is empty"));
        assert!(!err.to_string().contains("tier is not an object"));
    }

    #[test]
    fn tier_that_is_not_an_object_is_rejected() {
        for (name, body) in [
            (
                "tier-string.json",
                r#"{"currency":"EUR","tiers":["CANARY-SECRET-PR-TIERSTR"]}"#,
            ),
            ("tier-number.json", r#"{"currency":"EUR","tiers":[1]}"#),
            ("tier-null.json", r#"{"currency":"EUR","tiers":[null]}"#),
        ] {
            let err = load_body(name, body).unwrap_err();
            assert!(
                matches!(err, PriceError::TierNotObject { .. }),
                "{name}: {err}"
            );
            assert!(
                err.to_string().ends_with("tier is not an object"),
                "{name}: {err}"
            );
            assert!(!err.to_string().contains("tier name is empty"));
        }
    }

    #[test]
    fn negative_price_is_rejected() {
        let err = load_body(
            "negative.json",
            r#"{"currency":"EUR","tiers":[{"name":"CANARY-SECRET-PR-NEGNAME","price_per_rum_hour":-1.5}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, PriceError::NegativePrice { .. }));
        assert!(!err.to_string().contains("-1.5"));
        assert!(!err.to_string().contains("NEGNAME"));
    }

    #[test]
    fn non_numeric_price_is_rejected() {
        let err = load_body(
            "text-price.json",
            r#"{"currency":"EUR","tiers":[{"name":"small","price_per_rum_hour":"CANARY-SECRET-PR-TEXTPRICE"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, PriceError::NonNumericPrice { .. }));
        assert!(!err.to_string().contains("TEXTPRICE"));
    }

    #[test]
    fn empty_currency_is_rejected() {
        let err = load_body(
            "empty-currency.json",
            r#"{"currency":"","note":"CANARY-SECRET-PR-EMPTYCUR","tiers":[{"name":"small","price_per_rum_hour":1}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, PriceError::EmptyCurrency { .. }));
    }

    #[test]
    fn json_array_is_rejected_without_its_contents() {
        let err = load_body("array.json", r#"["CANARY-SECRET-PR-ARRAY"]"#).unwrap_err();
        assert!(matches!(err, PriceError::Invalid { .. }));
    }

    fn sample(currency: &str, tiers: &[(&str, f64)]) -> PriceList {
        PriceList {
            currency: currency.to_string(),
            tiers: tiers
                .iter()
                .map(|(name, price)| TierPrice {
                    name: (*name).to_string(),
                    price_per_rum_hour: *price,
                })
                .collect(),
        }
    }

    fn amounts(costs: &[TierCost]) -> Vec<(&str, &str, &str)> {
        costs
            .iter()
            .map(|cost| {
                (
                    cost.name.as_str(),
                    cost.currency.as_str(),
                    cost.amount.as_str(),
                )
            })
            .collect()
    }

    #[test]
    fn monthly_amount_is_rum_times_price_times_hours() {
        assert_eq!(DEFAULT_HOURS_PER_MONTH, 730);
        let costs = monthly_costs(2, &sample("EUR", &[("exact", 1.5), ("free", 0.0)]), 730)
            .expect("cost fits");
        assert_eq!(
            amounts(&costs),
            vec![("exact", "EUR", "2190.00"), ("free", "EUR", "0.00")]
        );
        assert_eq!(
            monthly_costs(0, &sample("EUR", &[("n", 4.0)]), 730).expect("cost fits")[0].amount,
            "0.00"
        );
        assert_eq!(
            monthly_costs(9, &sample("EUR", &[("n", 4.0)]), 0).expect("cost fits")[0].amount,
            "0.00"
        );
    }

    #[test]
    fn monthly_amount_rounds_half_away_from_zero_to_two_decimals() {
        // 0.125 → 0.13; 0.0625 → 0.06; 3 × 0.0625 × 10 = 1.875 → 1.88.
        assert_eq!(
            monthly_costs(1, &sample("EUR", &[("half", 0.125)]), 1).expect("cost fits")[0].amount,
            "0.13"
        );
        assert_eq!(
            monthly_costs(1, &sample("EUR", &[("down", 0.0625)]), 1).expect("cost fits")[0].amount,
            "0.06"
        );
        assert_eq!(
            monthly_costs(3, &sample("EUR", &[("product", 0.0625)]), 10).expect("cost fits")[0]
                .amount,
            "1.88"
        );
    }

    #[test]
    fn decimal_rounding_and_integer_precision_are_preserved() {
        assert_eq!(monthly_cents(1, 1.005, 1), Ok(101));
        assert_eq!(monthly_cents(1, 0.145, 1), Ok(15));
        assert_eq!(monthly_cents(1, 1.0049, 1), Ok(100));
        assert_eq!(monthly_cents(3, 0.005, 1), Ok(2));
        assert_eq!(monthly_cents(u64::MAX, 0.01, 1), Ok(u64::MAX));
        assert_eq!(monthly_cents(1, 0.01, u64::MAX), Ok(u64::MAX));
        assert_eq!(monthly_cents(u64::MAX, 0.01, 2), Err(CostOverflow));
    }

    #[test]
    fn extreme_prices_and_zero_factors() {
        assert_eq!(monthly_cents(2, 1e308, 0), Ok(0));
        assert_eq!(monthly_cents(0, 1e308, u64::MAX), Ok(0));
        assert_eq!(monthly_cents(u64::MAX, 1e-308, u64::MAX), Ok(0));
        assert_eq!(monthly_cents(1, 1e308, 1), Err(CostOverflow));
        for price in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(monthly_cents(0, price, 0), Err(CostOverflow));
        }
    }
}
