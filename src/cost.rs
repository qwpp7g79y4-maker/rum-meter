//! Tier prices from a user-supplied JSON file, and the monthly estimate.
//!
//! Shape: `{ "currency": "...", "tiers": [ { "name": "...", "price_per_rum_hour": n } ] }`.
//! Tier names are unique; a repeated name is [`PriceError::DuplicateTierName`].
//! `price_per_rum_hour` is the exact decimal from the JSON number, not a binary
//! float. A month is RUM × price per RUM-hour × hours (730 unless the caller
//! passes another value), rounded half away from zero to two decimal places.
//! A product that is negative, or that does not fit in the cent amount,
//! is [`CostOverflow`]: the amount is never clamped.
//! The program has no built-in prices. Error text is the path and the kind of
//! problem, never bytes from the file.
//!
//! Only a regular file is read. The file is opened first, without waiting on
//! Unix, and the type is taken from that descriptor before any byte is read.
//! A directory, a FIFO, a device, or a symlink to one of those is refused.
//! A symlink to a regular file is read.

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use bigdecimal::{BigDecimal, RoundingMode, ToPrimitive, Zero, num_bigint::Sign};
use serde::Deserialize;
use serde_json::value::RawValue;

/// Hours in the monthly estimate when `--hours-per-month` is absent.
pub const DEFAULT_HOURS_PER_MONTH: u64 = 730;

/// A price list read from disk. `price_per_rum_hour` is the exact decimal from the file.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceList {
    pub currency: String,
    pub tiers: Vec<TierPrice>,
}

/// One tier. The name and the price are taken from the file as given.
#[derive(Debug, Clone, PartialEq)]
pub struct TierPrice {
    pub name: String,
    /// Exact decimal from the JSON number. Not a binary float.
    pub price_per_rum_hour: BigDecimal,
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
                    &tier.price_per_rum_hour,
                    hours_per_month,
                )?),
            })
        })
        .collect()
}

/// Rounded monthly amount in cents. `diff` subtracts these integers.
///
/// `price_per_rum_hour` is the exact decimal from the file, so `1.005` rounds
/// to 1.01 and `0.004999999999999999999` stays below half a cent.
pub(crate) fn monthly_cents(
    rum: u64,
    price_per_rum_hour: &BigDecimal,
    hours_per_month: u64,
) -> Result<u64, CostOverflow> {
    if is_negative(price_per_rum_hour) {
        return Err(CostOverflow);
    }
    let scaled = BigDecimal::from(rum)
        * price_per_rum_hour.clone()
        * BigDecimal::from(hours_per_month)
        * BigDecimal::from(100u32);
    scaled
        .with_scale_round(0, RoundingMode::HalfUp)
        .to_u64()
        .ok_or(CostOverflow)
}

fn is_negative(price: &BigDecimal) -> bool {
    price.sign() == Sign::Minus && !Zero::is_zero(price)
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
    DuplicateKey {
        path: PathBuf,
    },
    /// Two tiers share a `name`. Nothing from either tier is printed.
    DuplicateTierName {
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
    DuplicateKey,
    DuplicateTierName,
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
            Self::DuplicateKey => PriceError::DuplicateKey { path },
            Self::DuplicateTierName => PriceError::DuplicateTierName { path },
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
            Self::DuplicateKey { path } => {
                write!(
                    f,
                    "rum-meter: {}: duplicate key in a tier",
                    crate::rum::escape_controls(&path.display().to_string())
                )
            }
            Self::DuplicateTierName { path } => {
                write!(
                    f,
                    "rum-meter: {}: duplicate tier name",
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
/// A path that is not a regular file is refused without reading a byte.
pub fn load_prices(path: &Path) -> Result<PriceList, PriceError> {
    let bytes = crate::regular::read_regular(path).map_err(|err| read_error(path, err))?;
    parse_bytes(&bytes).map_err(|err| err.with_path(path))
}

/// Fields kept as raw JSON so a price number is not parsed as `f64`.
#[derive(Deserialize)]
struct PriceFields<'a> {
    #[serde(default, borrow)]
    currency: Option<&'a RawValue>,
    #[serde(default, borrow)]
    tiers: Option<&'a RawValue>,
}

/// One tier, still raw. `name` and the price are checked after this.
#[derive(Deserialize)]
struct TierFields<'a> {
    #[serde(default, borrow)]
    name: Option<&'a RawValue>,
    #[serde(default, borrow)]
    price_per_rum_hour: Option<&'a RawValue>,
}

/// Parse price-file bytes directly, without touching the filesystem.
///
/// Exposed only under `cfg(fuzzing)` (set automatically by `cargo fuzz`), so the
/// fuzz crate can drive the parser without the normal public API growing a
/// bytes-in entry point.
#[cfg(fuzzing)]
pub fn parse_bytes_for_fuzzing(bytes: &[u8]) -> Result<PriceList, ()> {
    parse_bytes(bytes).map_err(|_| ())
}

/// One leading UTF-8 BOM is skipped; UTF-16 and UTF-32 are refused.
///
/// Prices stay raw JSON numbers until [`parse_price`], so a decimal such as
/// `0.004999999999999999999` is not rounded to a binary float first.
fn parse_bytes(bytes: &[u8]) -> Result<PriceList, ParseError> {
    let bytes = crate::encoding::utf8_body(bytes).map_err(ParseError::NotUtf8)?;
    let root: Box<RawValue> = match serde_json::from_slice(bytes) {
        Ok(root) => root,
        Err(err) => {
            return Err(ParseError::InvalidJson {
                line: err.line(),
                column: err.column(),
            });
        }
    };
    // A JSON array would otherwise fill these fields in order.
    if raw_is_array(root.get()) {
        return Err(ParseError::Invalid);
    }
    let fields: PriceFields<'_> =
        serde_json::from_str(root.get()).map_err(|_| ParseError::Invalid)?;
    let currency = match fields.currency {
        Some(raw) => match serde_json::from_str::<String>(raw.get()) {
            Ok(text) if !text.is_empty() => text,
            _ => return Err(ParseError::EmptyCurrency),
        },
        None => return Err(ParseError::EmptyCurrency),
    };
    let Some(tiers_raw) = fields.tiers else {
        return Err(ParseError::NoTiers);
    };
    let tiers: Vec<Box<RawValue>> =
        serde_json::from_str(tiers_raw.get()).map_err(|_| ParseError::NoTiers)?;
    if tiers.is_empty() {
        return Err(ParseError::NoTiers);
    }
    let mut parsed = Vec::with_capacity(tiers.len());
    let mut seen_names = std::collections::HashSet::with_capacity(tiers.len());
    for tier in &tiers {
        let tier = parse_tier(tier)?;
        if !seen_names.insert(tier.name.clone()) {
            return Err(ParseError::DuplicateTierName);
        }
        parsed.push(tier);
    }
    Ok(PriceList {
        currency,
        tiers: parsed,
    })
}

fn raw_is_array(raw: &str) -> bool {
    raw.bytes()
        .find(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
        == Some(b'[')
}

/// `true` when the tier JSON object repeats a top-level key (including the same
/// key written again after whitespace or as a `\uXXXX` escape that decodes to
/// an earlier key). Nested objects are ignored.
fn tier_has_duplicate_key(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(text) else {
        return false;
    };
    match count_top_level_object_keys(text) {
        Some(found) => found > value.len(),
        None => false,
    }
}

fn count_top_level_object_keys(text: &str) -> Option<usize> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut chars = text.chars().peekable();
    skip_ws(&mut chars);
    if chars.next() != Some('{') {
        return None;
    }
    skip_ws(&mut chars);
    if chars.peek() == Some(&'}') {
        chars.next();
        skip_ws(&mut chars);
        return chars.next().is_none().then_some(0);
    }
    let mut count = 0usize;
    loop {
        skip_ws(&mut chars);
        if !skip_json_string(&mut chars) {
            return None;
        }
        count = count.saturating_add(1);
        skip_ws(&mut chars);
        if chars.next() != Some(':') {
            return None;
        }
        if !skip_json_value(&mut chars) {
            return None;
        }
        skip_ws(&mut chars);
        match chars.next() {
            Some(',') => {}
            Some('}') => {
                skip_ws(&mut chars);
                return chars.next().is_none().then_some(count);
            }
            _ => return None,
        }
    }
}

fn skip_ws(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while matches!(chars.peek(), Some(' ' | '\t' | '\n' | '\r')) {
        chars.next();
    }
}

#[allow(clippy::collapsible_match)]
fn skip_json_string(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    if chars.next() != Some('"') {
        return false;
    }
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return true,
            '\\' => {
                if chars.next().is_none() {
                    return false;
                }
            }
            _ => {}
        }
    }
    false
}

fn skip_json_value(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    skip_ws(chars);
    match chars.peek().copied() {
        Some('"') => skip_json_string(chars),
        Some('{') => skip_json_container(chars, '{', '}'),
        Some('[') => skip_json_container(chars, '[', ']'),
        Some('t') => skip_literal(chars, "true"),
        Some('f') => skip_literal(chars, "false"),
        Some('n') => skip_literal(chars, "null"),
        Some('-') | Some('0'..='9') => {
            chars.next();
            while matches!(
                chars.peek().copied(),
                Some('0'..='9' | '.' | 'e' | 'E' | '+' | '-')
            ) {
                chars.next();
            }
            true
        }
        _ => false,
    }
}

fn skip_literal(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, literal: &str) -> bool {
    for expected in literal.chars() {
        if chars.next() != Some(expected) {
            return false;
        }
    }
    true
}

#[allow(clippy::collapsible_match)]
fn skip_json_container(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    open: char,
    close: char,
) -> bool {
    if chars.next() != Some(open) {
        return false;
    }
    let mut depth = 1usize;
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                // Put the quote back into the stream logic: we already consumed it.
                // Re-parse from after the quote by manually finishing the string.
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            if chars.next().is_none() {
                                return false;
                            }
                        }
                        Some(_) => {}
                        None => return false,
                    }
                }
            }
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn parse_tier(raw: &RawValue) -> Result<TierPrice, ParseError> {
    if raw_is_array(raw.get()) {
        return Err(ParseError::TierNotObject);
    }
    // serde_json with borrowed RawValue fails on a repeated key; map that to a
    // clear duplicate-key error instead of "tier is not an object".
    if tier_has_duplicate_key(raw.get()) {
        return Err(ParseError::DuplicateKey);
    }
    let fields: TierFields<'_> =
        serde_json::from_str(raw.get()).map_err(|_| ParseError::TierNotObject)?;
    let name = match fields.name {
        Some(raw) => match serde_json::from_str::<String>(raw.get()) {
            Ok(text) if !text.is_empty() => text,
            _ => return Err(ParseError::EmptyTierName),
        },
        None => return Err(ParseError::EmptyTierName),
    };
    let price = match fields.price_per_rum_hour {
        Some(raw) => parse_price(raw.get())?,
        None => return Err(ParseError::NonNumericPrice),
    };
    Ok(TierPrice {
        name,
        price_per_rum_hour: price,
    })
}

/// `raw` is one JSON value. A number is kept digit for digit. Anything else,
/// including a string, is not a price.
fn parse_price(raw: &str) -> Result<BigDecimal, ParseError> {
    let text = raw.trim();
    let digits = text.strip_prefix('-').unwrap_or(text);
    if !digits.starts_with(|ch: char| ch.is_ascii_digit()) {
        return Err(ParseError::NonNumericPrice);
    }
    let price = BigDecimal::from_str(text).map_err(|_| ParseError::NonNumericPrice)?;
    if is_negative(&price) {
        return Err(ParseError::NegativePrice);
    }
    Ok(price)
}

fn read_error(path: &Path, err: crate::regular::RegularError) -> PriceError {
    match err {
        crate::regular::RegularError::NotRegular => PriceError::NotRegular {
            path: path.to_path_buf(),
        },
        crate::regular::RegularError::Unreadable { kind } => PriceError::Unreadable {
            path: path.to_path_buf(),
            kind,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::BigDecimal;
    use std::fs;
    use std::str::FromStr;

    const CANARY: &str = "CANARY-SECRET-PR";

    fn dec(text: &str) -> BigDecimal {
        BigDecimal::from_str(text).unwrap_or_else(|_| panic!("decimal {text}"))
    }

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
        assert_eq!(prices.tiers[0].price_per_rum_hour, dec("1.25"));
        assert_eq!(prices.tiers[1].name, "large");
        assert_eq!(prices.tiers[1].price_per_rum_hour, dec("0"));
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
    fn duplicate_key_in_a_tier_is_refused_with_a_clear_message() {
        // Reported: repeated price_per_rum_hour → used to say "tier is not an object".
        let cases = [
            (
                "dup-price.json",
                r#"{"currency":"EUR","tiers":[{"name":"t","price_per_rum_hour":1,"price_per_rum_hour":999}]}"#,
            ),
            (
                "dup-price-reversed.json",
                r#"{"currency":"EUR","tiers":[{"price_per_rum_hour":1,"name":"t","price_per_rum_hour":999}]}"#,
            ),
            (
                "dup-price-ws.json",
                r#"{"currency":"EUR","tiers":[{ "name" : "t" , "price_per_rum_hour" : 1 , "price_per_rum_hour" : 999 }]}"#,
            ),
            (
                "dup-price-lf.json",
                "{\"currency\":\"EUR\",\"tiers\":[{\"name\":\"t\",\"price_per_rum_hour\":1,\n\"price_per_rum_hour\":999}]}",
            ),
            (
                "dup-price-crlf.json",
                "{\"currency\":\"EUR\",\"tiers\":[{\"name\":\"t\",\"price_per_rum_hour\":1,\r\n\"price_per_rum_hour\":999}]}",
            ),
            (
                "dup-price-tab.json",
                "{\"currency\":\"EUR\",\"tiers\":[{\"name\":\"t\",\"price_per_rum_hour\":1,\t\"price_per_rum_hour\":999}]}",
            ),
            (
                "dup-price-uescape.json",
                r#"{"currency":"EUR","tiers":[{"name":"t","price_per_rum_hour":1,"\u0070rice_per_rum_hour":999}]}"#,
            ),
            (
                "dup-name.json",
                r#"{"currency":"EUR","tiers":[{"name":"a","name":"b","price_per_rum_hour":1}]}"#,
            ),
        ];
        for (name, body) in cases {
            let err = load_body(name, body).unwrap_err();
            assert!(
                matches!(err, PriceError::DuplicateKey { .. }),
                "{name}: {err}"
            );
            assert!(
                err.to_string().ends_with("duplicate key in a tier"),
                "{name}: {err}"
            );
            assert!(
                !err.to_string().contains("tier is not an object"),
                "{name}: still the old message: {err}"
            );
            assert!(!err.to_string().contains("999"));
            assert!(!err.to_string().contains("CANARY"));
        }
    }

    #[test]
    fn duplicate_tier_name_is_rejected() {
        let err = load_body(
            "dup-tier-name.json",
            r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":1},{"name":"a","price_per_rum_hour":2}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, PriceError::DuplicateTierName { .. }));
        assert!(err.to_string().ends_with("duplicate tier name"));
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

    fn sample(currency: &str, tiers: &[(&str, &str)]) -> PriceList {
        PriceList {
            currency: currency.to_string(),
            tiers: tiers
                .iter()
                .map(|(name, price)| TierPrice {
                    name: (*name).to_string(),
                    price_per_rum_hour: dec(price),
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
        let costs = monthly_costs(2, &sample("EUR", &[("exact", "1.5"), ("free", "0")]), 730)
            .expect("cost fits");
        assert_eq!(
            amounts(&costs),
            vec![("exact", "EUR", "2190.00"), ("free", "EUR", "0.00")]
        );
        assert_eq!(
            monthly_costs(0, &sample("EUR", &[("n", "4")]), 730).expect("cost fits")[0].amount,
            "0.00"
        );
        assert_eq!(
            monthly_costs(9, &sample("EUR", &[("n", "4")]), 0).expect("cost fits")[0].amount,
            "0.00"
        );
    }

    #[test]
    fn monthly_amount_rounds_half_away_from_zero_to_two_decimals() {
        // 0.125 → 0.13; 0.0625 → 0.06; 3 × 0.0625 × 10 = 1.875 → 1.88.
        assert_eq!(
            monthly_costs(1, &sample("EUR", &[("half", "0.125")]), 1).expect("cost fits")[0].amount,
            "0.13"
        );
        assert_eq!(
            monthly_costs(1, &sample("EUR", &[("down", "0.0625")]), 1).expect("cost fits")[0]
                .amount,
            "0.06"
        );
        assert_eq!(
            monthly_costs(3, &sample("EUR", &[("product", "0.0625")]), 10).expect("cost fits")[0]
                .amount,
            "1.88"
        );
    }

    #[test]
    fn decimal_rounding_and_integer_precision_are_preserved() {
        assert_eq!(monthly_cents(1, &dec("1.005"), 1), Ok(101));
        assert_eq!(monthly_cents(1, &dec("0.145"), 1), Ok(15));
        assert_eq!(monthly_cents(1, &dec("1.0049"), 1), Ok(100));
        assert_eq!(monthly_cents(3, &dec("0.005"), 1), Ok(2));
        assert_eq!(monthly_cents(u64::MAX, &dec("0.01"), 1), Ok(u64::MAX));
        assert_eq!(monthly_cents(1, &dec("0.01"), u64::MAX), Ok(u64::MAX));
        assert_eq!(monthly_cents(u64::MAX, &dec("0.01"), 2), Err(CostOverflow));
    }

    #[test]
    fn price_below_half_a_cent_stays_exact() {
        let body = r#"{"currency":"EUR","tiers":[{"name":"tier","price_per_rum_hour":0.004999999999999999999}]}"#;
        let prices = load_body("below-half-cent.json", body).expect("price");
        assert_eq!(
            monthly_cents(1, &prices.tiers[0].price_per_rum_hour, 1),
            Ok(0)
        );
        assert_eq!(monthly_costs(1, &prices, 1).unwrap()[0].amount, "0.00");
    }

    #[test]
    fn extreme_prices_and_zero_factors() {
        assert_eq!(monthly_cents(2, &dec("1e308"), 0), Ok(0));
        assert_eq!(monthly_cents(0, &dec("1e308"), u64::MAX), Ok(0));
        assert_eq!(monthly_cents(u64::MAX, &dec("1e-308"), u64::MAX), Ok(0));
        assert_eq!(monthly_cents(1, &dec("1e308"), 1), Err(CostOverflow));
        assert_eq!(monthly_cents(0, &dec("-1"), 0), Err(CostOverflow));
    }
}
