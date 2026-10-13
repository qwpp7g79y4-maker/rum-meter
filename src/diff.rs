//! Difference between two classified states.
//!
//! Identity is the instance address, which includes the resource type and, for
//! a deposed object, the deposed key (` (deposed KEY)`). Added and removed
//! list counting addresses only. A rule change is the same address in both
//! states with a different rule, and that address is not repeated in added or
//! removed. Because the type is part of the address, a resource that becomes
//! `terraform_data` leaves the counting set as a removal. A managed resource
//! that becomes a data source does the same: its address starts with `data.`
//! and is a different object. The tainted flag is not part of the address.
//!
//! Cost delta is the new monthly tier amount minus the old one. Both sides
//! are the integer cents from [`crate::cost::monthly_cents`], the same
//! rounding [`crate::cost::monthly_costs`] prints. The formatted text is not parsed.

use std::collections::BTreeMap;

use crate::cost::{self, PriceList};
use crate::rum::{Rule, Verdict, total_rum};

/// Same address, different rule. The address is in both states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleChange {
    pub address: String,
    pub old_rule: Rule,
    pub new_rule: Rule,
}

/// Billable difference between an older state and a newer one.
///
/// `added`, `removed` and `changed_rule` are sorted by address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed_rule: Vec<RuleChange>,
    pub old_rum: usize,
    pub new_rum: usize,
}

impl StateDiff {
    /// New RUM minus old RUM. Positive when the new state counts more.
    pub fn rum_delta(&self) -> i64 {
        let old = i64::try_from(self.old_rum).unwrap_or(i64::MAX);
        let new = i64::try_from(self.new_rum).unwrap_or(i64::MAX);
        new.saturating_sub(old)
    }
}

/// One tier of the monthly cost difference. `amount` is a signed two-decimal number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostDelta {
    pub name: String,
    pub currency: String,
    pub amount: String,
}

/// Compare two classified states. Duplicate addresses in one state keep the first verdict.
pub fn diff_verdicts(old: &[Verdict], new: &[Verdict]) -> StateDiff {
    let old_by_address = by_address(old);
    let new_by_address = by_address(new);

    let mut added = Vec::new();
    let mut changed_rule = Vec::new();
    for (address, new_verdict) in &new_by_address {
        let address = *address;
        match old_by_address.get(address) {
            None if new_verdict.counts => added.push(address.to_string()),
            Some(old_verdict) if old_verdict.rule != new_verdict.rule => {
                changed_rule.push(RuleChange {
                    address: address.to_string(),
                    old_rule: old_verdict.rule,
                    new_rule: new_verdict.rule,
                });
            }
            _ => {}
        }
    }

    let mut removed = Vec::new();
    for (address, old_verdict) in &old_by_address {
        let address = *address;
        if old_verdict.counts && !new_by_address.contains_key(address) {
            removed.push(address.to_string());
        }
    }

    StateDiff {
        added,
        removed,
        changed_rule,
        old_rum: total_rum(old),
        new_rum: total_rum(new),
    }
}

/// Per-tier difference of the two monthly estimates, in price-file order.
///
/// Each side is rounded to cents on its own, then subtracted as an integer.
pub fn cost_deltas(
    old_rum: u64,
    new_rum: u64,
    prices: &PriceList,
    hours_per_month: u64,
) -> Result<Vec<CostDelta>, cost::CostOverflow> {
    prices
        .tiers
        .iter()
        .map(|tier| {
            let old_cents =
                cost::monthly_cents(old_rum, &tier.price_per_rum_hour, hours_per_month)?;
            let new_cents =
                cost::monthly_cents(new_rum, &tier.price_per_rum_hour, hours_per_month)?;
            Ok(CostDelta {
                name: tier.name.clone(),
                currency: prices.currency.clone(),
                amount: format_signed_cents(i128::from(new_cents) - i128::from(old_cents)),
            })
        })
        .collect()
}

fn by_address(verdicts: &[Verdict]) -> BTreeMap<&str, &Verdict> {
    let mut map = BTreeMap::new();
    for verdict in verdicts {
        map.entry(verdict.address.as_str()).or_insert(verdict);
    }
    map
}

fn format_signed_cents(cents: i128) -> String {
    let sign = match cents.signum() {
        1 => "+",
        -1 => "-",
        _ => "",
    };
    let abs = cents.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cost::{PriceList, TierPrice};
    use crate::rum::{Rule, classify};
    use crate::state::{IndexKey, Instance, Resource, State, load_state};
    use bigdecimal::BigDecimal;
    use std::path::PathBuf;
    use std::str::FromStr;

    fn verdict(address: &str, rule: Rule) -> Verdict {
        Verdict {
            address: address.to_string(),
            resource_type: "random_id".to_string(),
            module: None,
            counts: rule == Rule::R1,
            rule,
            reason: rule.reason(),
            tainted: false,
            deposed: None,
        }
    }

    fn prices(tiers: &[(&str, &str)]) -> PriceList {
        PriceList {
            currency: "EUR".to_string(),
            tiers: tiers
                .iter()
                .map(|(name, price)| TierPrice {
                    name: (*name).to_string(),
                    price_per_rum_hour: BigDecimal::from_str(price).expect("price"),
                })
                .collect(),
        }
    }

    fn amounts(deltas: &[CostDelta]) -> Vec<(&str, &str)> {
        deltas
            .iter()
            .map(|delta| (delta.name.as_str(), delta.amount.as_str()))
            .collect()
    }

    fn resource(
        mode: &str,
        resource_type: &str,
        name: &str,
        module: Option<&str>,
        index_keys: &[Option<IndexKey>],
    ) -> Resource {
        Resource {
            mode: mode.to_string(),
            resource_type: resource_type.to_string(),
            name: name.to_string(),
            module: module.map(str::to_string),
            instances: index_keys
                .iter()
                .cloned()
                .map(|index_key| Instance {
                    index_key,
                    deposed: None,
                    tainted: false,
                })
                .collect(),
        }
    }

    fn state(resources: Vec<Resource>) -> State {
        State {
            version: 4,
            resources,
        }
    }

    #[test]
    fn count_add_for_each_removal_and_terraform_data_change_the_rum() {
        let old = state(vec![
            resource(
                "managed",
                "random_id",
                "servers",
                None,
                &[Some(IndexKey::Number(0))],
            ),
            resource(
                "managed",
                "random_id",
                "labeled",
                Some("module.child"),
                &[
                    Some(IndexKey::String("web".to_string())),
                    Some(IndexKey::String("gone".to_string())),
                ],
            ),
            resource("managed", "random_id", "marker", None, &[None]),
            resource("managed", "local_file", "note", None, &[None]),
        ]);
        let new = state(vec![
            resource(
                "managed",
                "random_id",
                "servers",
                None,
                &[Some(IndexKey::Number(0)), Some(IndexKey::Number(1))],
            ),
            resource(
                "managed",
                "random_id",
                "labeled",
                Some("module.child"),
                &[Some(IndexKey::String("web".to_string()))],
            ),
            resource("managed", "terraform_data", "marker", None, &[None]),
            resource("data", "local_file", "note", None, &[None]),
        ]);
        let diff = diff_verdicts(&classify(&old), &classify(&new));
        assert_eq!(diff.old_rum, 5);
        assert_eq!(diff.new_rum, 3);
        assert_eq!(diff.rum_delta(), -2);
        assert_eq!(diff.added, vec!["random_id.servers[1]".to_string()]);
        assert_eq!(
            diff.removed,
            vec![
                "local_file.note".to_string(),
                "module.child.random_id.labeled[\"gone\"]".to_string(),
                "random_id.marker".to_string(),
            ]
        );
        assert!(diff.changed_rule.is_empty());
        assert!(
            !diff
                .added
                .iter()
                .any(|address| address.contains("terraform_data"))
        );
    }

    #[test]
    fn rule_change_is_not_listed_as_added_or_removed() {
        let old = vec![
            verdict("random_id.absent", Rule::R4),
            verdict("local_file.note", Rule::R1),
        ];
        let new = vec![
            verdict("random_id.absent", Rule::R1),
            verdict("local_file.note", Rule::R2),
        ];
        let diff = diff_verdicts(&old, &new);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.old_rum, 1);
        assert_eq!(diff.new_rum, 1);
        assert_eq!(diff.rum_delta(), 0);
        assert_eq!(
            diff.changed_rule,
            vec![
                RuleChange {
                    address: "local_file.note".to_string(),
                    old_rule: Rule::R1,
                    new_rule: Rule::R2,
                },
                RuleChange {
                    address: "random_id.absent".to_string(),
                    old_rule: Rule::R4,
                    new_rule: Rule::R1,
                },
            ]
        );
    }

    #[test]
    fn non_counting_only_addresses_are_omitted_and_lists_are_sorted() {
        let old = vec![
            verdict("z", Rule::R1),
            verdict("m.data", Rule::R2),
            verdict("b", Rule::R1),
        ];
        let new = vec![
            verdict("m", Rule::R1),
            verdict("a", Rule::R1),
            verdict("fresh.data", Rule::R2),
            verdict("b", Rule::R1),
        ];
        let diff = diff_verdicts(&old, &new);
        assert_eq!(diff.added, vec!["a".to_string(), "m".to_string()]);
        assert_eq!(diff.removed, vec!["z".to_string()]);
        assert!(diff.changed_rule.is_empty());
        assert_eq!(diff.old_rum, 2);
        assert_eq!(diff.new_rum, 3);
        assert_eq!(diff.rum_delta(), 1);
    }

    #[test]
    fn identical_verdicts_have_an_empty_diff() {
        let verdicts = vec![verdict("random_id.steady", Rule::R1)];
        let diff = diff_verdicts(&verdicts, &verdicts);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.changed_rule.is_empty());
        assert_eq!(diff.rum_delta(), 0);
    }

    #[test]
    fn cost_delta_subtracts_the_rounded_monthly_amounts() {
        let list = prices(&[("alpha", "1.25"), ("beta", "0.5")]);
        assert_eq!(
            amounts(&cost_deltas(6, 4, &list, 730).expect("cost fits")),
            vec![("alpha", "-1825.00"), ("beta", "-730.00")]
        );
        assert_eq!(
            amounts(&cost_deltas(6, 4, &list, 10).expect("cost fits")),
            vec![("alpha", "-25.00"), ("beta", "-10.00")]
        );
        assert_eq!(
            amounts(&cost_deltas(2, 2, &list, 730).expect("cost fits")),
            vec![("alpha", "0.00"), ("beta", "0.00")]
        );
        // 1 × 0.0625 → 0.06 and 2 × 0.0625 → 0.13, so the bill moves by +0.07.
        let awkward = prices(&[("half", "0.0625")]);
        assert_eq!(
            amounts(&cost_deltas(1, 2, &awkward, 1).expect("cost fits")),
            vec![("half", "+0.07")]
        );
    }

    #[test]
    fn fixture_states_keep_canaries_out_of_the_diff() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let old = load_state(&root.join("tests/fixtures/v4_diff_old.json")).expect("old fixture");
        let new = load_state(&root.join("tests/fixtures/v4_diff_new.json")).expect("new fixture");
        let diff = diff_verdicts(&classify(&old), &classify(&new));
        assert_eq!(diff.old_rum, 6);
        assert_eq!(diff.new_rum, 4);
        assert_eq!(diff.rum_delta(), -2);
        assert_eq!(diff.added, vec!["random_id.servers[1]".to_string()]);
        assert_eq!(
            diff.removed,
            vec![
                "local_file.note".to_string(),
                "module.child.random_id.labeled[\"gone\"]".to_string(),
                "random_id.marker".to_string(),
            ]
        );
        assert!(diff.changed_rule.is_empty());
        let rendered = format!("{old:?}{new:?}{diff:?}");
        assert!(!rendered.contains("CANARY-SECRET"));
    }

    #[test]
    fn deposed_objects_keep_added_minus_removed_equal_to_rum_delta() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let new = load_state(&root.join("tests/fixtures/v4_object_status.json")).expect("fixture");
        let old = state(vec![resource("managed", "local_file", "a", None, &[None])]);
        let diff = diff_verdicts(&classify(&old), &classify(&new));
        let added = i64::try_from(diff.added.len()).expect("added len");
        let removed = i64::try_from(diff.removed.len()).expect("removed len");
        assert_eq!(added - removed, diff.rum_delta());
        assert_eq!(diff.old_rum, 1);
        assert_eq!(diff.new_rum, 3);
        assert_eq!(diff.rum_delta(), 2);
        assert_eq!(
            diff.added,
            vec![
                "local_file.a (deposed 00000001)".to_string(),
                "local_file.b".to_string(),
            ]
        );
        assert!(diff.removed.is_empty());
        assert!(diff.changed_rule.is_empty());
        let rendered = format!("{old:?}{new:?}{diff:?}");
        assert!(!rendered.contains("CANARY-SECRET"));
    }

    #[test]
    fn tainted_flag_does_not_change_address_identity() {
        let mut old = verdict("local_file.b", Rule::R1);
        old.tainted = true;
        let new = verdict("local_file.b", Rule::R1);
        let diff = diff_verdicts(&[old], &[new]);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert!(diff.changed_rule.is_empty());
        assert_eq!(diff.rum_delta(), 0);
    }

    #[test]
    fn cost_deltas_do_not_parse_formatted_amounts() {
        let src = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/diff.rs"));
        let body = src.split_once("mod tests").expect("test module").0;
        assert!(
            !body.contains("parse_amount_cents"),
            "diff cost math parses a formatted amount"
        );
        assert!(
            !body.contains("split_once"),
            "diff cost math parses a formatted amount"
        );
    }
}
