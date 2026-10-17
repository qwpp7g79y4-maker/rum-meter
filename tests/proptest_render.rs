//! Property test: rendering the same verdicts twice is byte-for-byte stable,
//! the JSON report always parses, and no field value reaches the text report
//! with a character `escape_controls` would still want to escape.

use std::path::Path;

use proptest::prelude::*;
use rum_meter::rum::{Rule, Verdict, escape_controls};

fn arbitrary_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..16)
        .prop_map(|chars| chars.into_iter().collect::<String>())
}

fn rule_from(n: u8) -> Rule {
    match n % 4 {
        0 => Rule::R1,
        1 => Rule::R2,
        2 => Rule::R3,
        _ => Rule::R4,
    }
}

fn build_verdicts(addresses: &[String], rule_bytes: &[u8]) -> Vec<Verdict> {
    addresses
        .iter()
        .enumerate()
        .map(|(i, address)| {
            let rule = rule_from(*rule_bytes.get(i).unwrap_or(&0));
            let address = if address.is_empty() {
                "x".to_string()
            } else {
                address.clone()
            };
            Verdict {
                address: address.clone(),
                identity: format!("{i}:{address}"),
                resource_type: "random_id".to_string(),
                module: None,
                counts: rule == Rule::R1,
                rule,
                reason: rule.reason(),
                tainted: i % 2 == 0,
                deposed: None,
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn count_report_is_stable_escaped_and_json_parses(
        addresses in proptest::collection::vec(arbitrary_string(), 0..8),
        rule_bytes in proptest::collection::vec(any::<u8>(), 0..8),
        top in proptest::option::of(0u64..5),
    ) {
        let verdicts = build_verdicts(&addresses, &rule_bytes);
        let path = Path::new("proptest-count.json");
        let files = [rum_meter::render::CountedFile { path, verdicts: &verdicts }];

        let text_a = rum_meter::render::render_count_text(&files, true, top, &[]);
        let text_b = rum_meter::render::render_count_text(&files, true, top, &[]);
        prop_assert_eq!(&text_a, &text_b, "rendering the same verdicts twice gave different text");
        for line in text_a.lines() {
            let escaped = escape_controls(line);
            prop_assert_eq!(
                escaped.as_ref(),
                line,
                "count text has an unescaped control/format character"
            );
        }

        let json_a = rum_meter::render::render_count_json(&files, true, top, &[]);
        let json_b = rum_meter::render::render_count_json(&files, true, top, &[]);
        prop_assert_eq!(&json_a, &json_b, "rendering the same verdicts twice gave different json");
        let parsed: serde_json::Value =
            serde_json::from_str(&json_a).expect("count json report must always parse");
        prop_assert!(parsed.is_object());
    }

    #[test]
    fn diff_report_is_stable_escaped_and_json_parses(
        old_addresses in proptest::collection::vec(arbitrary_string(), 0..6),
        new_addresses in proptest::collection::vec(arbitrary_string(), 0..6),
        rule_bytes in proptest::collection::vec(any::<u8>(), 0..6),
        top in proptest::option::of(0u64..5),
    ) {
        let old_verdicts = build_verdicts(&old_addresses, &rule_bytes);
        let new_verdicts = build_verdicts(&new_addresses, &rule_bytes);
        let diff = rum_meter::diff::diff_verdicts(&old_verdicts, &new_verdicts);
        let old_path = Path::new("proptest-old.json");
        let new_path = Path::new("proptest-new.json");

        let text_a = rum_meter::render::render_diff_text(old_path, new_path, &diff, &[]);
        let text_b = rum_meter::render::render_diff_text(old_path, new_path, &diff, &[]);
        prop_assert_eq!(&text_a, &text_b, "rendering the same diff twice gave different text");
        for line in text_a.lines() {
            let escaped = escape_controls(line);
            prop_assert_eq!(
                escaped.as_ref(),
                line,
                "diff text has an unescaped control/format character"
            );
        }

        let input = rum_meter::render::DiffJsonInput {
            old: old_path,
            new: new_path,
            old_verdicts: &old_verdicts,
            new_verdicts: &new_verdicts,
            diff: &diff,
            costs: &[],
            cost_deltas: &[],
        };
        let json_a = rum_meter::render::render_diff_json(&input, true, top);
        let json_b = rum_meter::render::render_diff_json(&input, true, top);
        prop_assert_eq!(&json_a, &json_b, "rendering the same diff twice gave different json");
        let parsed: serde_json::Value =
            serde_json::from_str(&json_a).expect("diff json report must always parse");
        prop_assert!(parsed.is_object());
    }
}
