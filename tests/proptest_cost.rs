//! Property test: `monthly_costs` matches a straightforward, independently
//! computed rational-arithmetic reference, not just the implementation's own
//! `BigDecimal` rounding call.
//!
//! The reference builds the exact fraction `rum * hours * 100 * price` as a
//! ratio of two `u128` integers (no floating point, no `BigDecimal`) and
//! rounds it half away from zero by hand.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use proptest::prelude::*;
use rum_meter::cost::{PriceList, TierPrice, monthly_costs};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn monthly_cost_matches_exact_rational_reference(
        rum in 0u64..10_000,
        hours in 0u64..10_000,
        int_part in 0u64..10_000,
        frac_digits in 0u32..=6,
        frac_value in 0u64..1_000_000,
    ) {
        let frac_scale = 10u64.pow(frac_digits);
        let frac = frac_value % frac_scale;
        let price_text = if frac_digits == 0 {
            int_part.to_string()
        } else {
            format!("{int_part}.{:0width$}", frac, width = frac_digits as usize)
        };
        let price = BigDecimal::from_str(&price_text).expect("constructed decimal parses");
        let prices = PriceList {
            currency: "EUR".to_string(),
            tiers: vec![TierPrice {
                name: "t".to_string(),
                price_per_rum_hour: price,
            }],
        };

        // Exact fraction: numerator / denominator == price_per_rum_hour.
        let numerator = u128::from(int_part) * u128::from(frac_scale) + u128::from(frac);
        let denominator = u128::from(frac_scale);
        let n = u128::from(rum) * u128::from(hours) * 100 * numerator;
        // Round half away from zero (n and denominator are both non-negative here).
        let reference_cents = (n * 2 + denominator) / (denominator * 2);

        match monthly_costs(rum, &prices, hours) {
            Ok(costs) => {
                let amount = &costs[0].amount;
                let (whole, frac_part) = amount
                    .split_once('.')
                    .expect("amount is always of the form N.NN");
                let cents = whole.parse::<u128>().expect("whole part is numeric") * 100
                    + frac_part.parse::<u128>().expect("fractional part is numeric");
                prop_assert_eq!(
                    cents,
                    reference_cents,
                    "monthly_costs diverged from the exact rational reference"
                );
                prop_assert!(!amount.starts_with('-'), "monthly amount went negative");
            }
            Err(_) => {
                prop_assert!(
                    reference_cents > u128::from(u64::MAX),
                    "monthly_costs reported overflow below u64::MAX"
                );
            }
        }
    }
}
