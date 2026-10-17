#![no_main]

use std::str::FromStr;

use bigdecimal::BigDecimal;
use libfuzzer_sys::fuzz_target;
use rum_meter::cost::{PriceList, TierPrice, monthly_costs};

fuzz_target!(|data: &[u8]| {
    if data.len() < 16 {
        return;
    }
    let rum = u64::from_le_bytes(data[0..8].try_into().expect("8 bytes"));
    let hours = u64::from_le_bytes(data[8..16].try_into().expect("8 bytes"));
    let Ok(price) = BigDecimal::from_str(&decimal_like(&data[16..])) else {
        return;
    };
    if price.sign() == bigdecimal::num_bigint::Sign::Minus {
        // A negative price is documented to be `CostOverflow`; covered elsewhere.
        return;
    }

    let prices = PriceList {
        currency: "EUR".to_string(),
        tiers: vec![TierPrice {
            name: "t".to_string(),
            price_per_rum_hour: price,
        }],
    };

    // Overflow is an explicit, documented error: never a negative/NaN amount.
    if let Ok(costs) = monthly_costs(rum, &prices, hours) {
        let amount = &costs[0].amount;
        assert!(
            !amount.starts_with('-'),
            "negative monthly amount for rum={rum} hours={hours}: {amount}"
        );
        let (whole, frac) = amount
            .split_once('.')
            .unwrap_or_else(|| panic!("amount {amount} has no decimal point"));
        assert_eq!(frac.len(), 2, "amount {amount} does not have two decimals");
        assert!(
            whole.chars().all(|c| c.is_ascii_digit()) && frac.chars().all(|c| c.is_ascii_digit()),
            "amount {amount} is not purely numeric"
        );
        let value: f64 = amount
            .parse()
            .unwrap_or_else(|_| panic!("amount {amount} did not parse as a number"));
        assert!(value.is_finite(), "amount {amount} is not finite");
    }
});

/// Turn arbitrary bytes into a string that looks enough like a JSON number for
/// `BigDecimal::from_str` to sometimes accept: digits, a dot, or an exponent marker.
fn decimal_like(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes.iter().take(40) {
        match b % 13 {
            0..=9 => out.push((b'0' + (b % 10)) as char),
            10 => out.push('.'),
            11 => out.push('-'),
            _ => out.push('e'),
        }
    }
    out
}
