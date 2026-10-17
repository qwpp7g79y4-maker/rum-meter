#![no_main]

use bigdecimal::BigDecimal;
use libfuzzer_sys::fuzz_target;
use rum_meter::cost::{monthly_costs, parse_bytes_for_fuzzing as parse_price_bytes};

fuzz_target!(|data: &[u8]| {
    // Arbitrary bytes must never panic the price-tier (allow-list) parser.
    let Ok(prices) = parse_price_bytes(data) else {
        return;
    };

    // Promises `parse_tier` / `parse_bytes` make about anything that parsed.
    assert!(!prices.currency.is_empty(), "empty currency should be Err");
    assert!(!prices.tiers.is_empty(), "empty tier list should be Err");
    for tier in &prices.tiers {
        assert!(!tier.name.is_empty(), "empty tier name should be Err");
        assert!(
            tier.price_per_rum_hour >= BigDecimal::from(0),
            "a negative price should be Err, got {}",
            tier.price_per_rum_hour
        );
    }

    // Cost calculation downstream of an accepted price file must stay sane.
    if let Ok(costs) = monthly_costs(1_000_000, &prices, 730) {
        for cost in &costs {
            assert!(
                !cost.amount.starts_with('-'),
                "monthly amount went negative: {}",
                cost.amount
            );
            let value: f64 = cost
                .amount
                .parse()
                .unwrap_or_else(|_| panic!("amount {} is not a plain decimal", cost.amount));
            assert!(value.is_finite(), "amount {} is not finite", cost.amount);
        }
    }
});
