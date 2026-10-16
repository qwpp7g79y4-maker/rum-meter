//! Billable RUM by module and by resource type.
//!
//! Only verdicts that count are included. Rows are ordered by count descending,
//! then by name ascending. `top` keeps that many rows per list and merges the
//! rest into one final remainder row named `other`. When a real module or type
//! is already named `other`, the remainder is `(other)` (then `(other 2)`, …)
//! so every name in the list stays unique. Without `top`, or when nothing is
//! left over, there is no remainder row. An absent or empty module is `(root)`.

use std::collections::BTreeMap;

use crate::rum::Verdict;

const OTHER: &str = "other";
const ROOT_MODULE: &str = "(root)";

/// One row of a breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub count: usize,
}

/// Billable instances grouped by module and by resource type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakdown {
    pub modules: Vec<Row>,
    pub types: Vec<Row>,
}

/// Group counting verdicts. The same module path or type is summed.
pub fn breakdown<'a>(
    verdicts: impl IntoIterator<Item = &'a Verdict>,
    top: Option<u64>,
) -> Breakdown {
    let mut modules = BTreeMap::<String, usize>::new();
    let mut types = BTreeMap::<String, usize>::new();
    for verdict in verdicts {
        if !verdict.counts {
            continue;
        }
        *modules
            .entry(module_label(verdict.module.as_deref()).to_string())
            .or_default() += 1;
        *types.entry(verdict.resource_type.clone()).or_default() += 1;
    }
    Breakdown {
        modules: ranked(modules, top),
        types: ranked(types, top),
    }
}

fn module_label(module: Option<&str>) -> &str {
    match module {
        Some(path) if !path.is_empty() => path,
        _ => ROOT_MODULE,
    }
}

fn ranked(counts: BTreeMap<String, usize>, top: Option<u64>) -> Vec<Row> {
    let mut rows: Vec<Row> = counts
        .into_iter()
        .map(|(name, count)| Row { name, count })
        .collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.name.cmp(&right.name))
    });
    let Some(top) = top else {
        return rows;
    };
    let Ok(top) = usize::try_from(top) else {
        return rows;
    };
    if top >= rows.len() {
        return rows;
    }
    let rest = rows[top..].iter().map(|row| row.count).sum();
    rows.truncate(top);
    rows.push(Row {
        name: remainder_label(&rows),
        count: rest,
    });
    rows
}

/// Name for the merged remainder. Prefer `other`; when that name is already a
/// real row (EC1: same label twice would be ambiguous), use `(other)`, then
/// `(other 2)`, and so on.
fn remainder_label(kept: &[Row]) -> String {
    if !kept.iter().any(|row| row.name == OTHER) {
        return OTHER.to_string();
    }
    let mut label = String::from("(other)");
    let mut n = 2u32;
    while kept.iter().any(|row| row.name == label) {
        label = format!("(other {n})");
        n = n.saturating_add(1);
        if n == 0 {
            break;
        }
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rum::Rule;

    fn row(name: &str, count: usize) -> Row {
        Row {
            name: name.to_string(),
            count,
        }
    }

    fn counting(module: Option<&str>, resource_type: &str) -> Verdict {
        Verdict {
            address: String::new(),
            identity: String::new(),
            resource_type: resource_type.to_string(),
            module: module.map(str::to_string),
            counts: true,
            rule: Rule::R1,
            reason: Rule::R1.reason(),
            tainted: false,
            deposed: None,
        }
    }

    fn sum(rows: &[Row]) -> usize {
        rows.iter().map(|row| row.count).sum()
    }

    #[test]
    fn higher_counts_come_first_and_ties_break_alphabetically() {
        // Insertion order is the opposite of the alphabetical tie-break.
        let verdicts = vec![
            counting(Some("module.b"), "zeta"),
            counting(Some("module.b"), "zeta"),
            counting(Some("module.c"), "mid"),
            counting(Some("module.a"), "alpha"),
            counting(Some("module.a"), "alpha"),
        ];
        let full = breakdown(&verdicts, None);
        assert_eq!(
            full.modules,
            vec![row("module.a", 2), row("module.b", 2), row("module.c", 1)]
        );
        assert_eq!(
            full.types,
            vec![row("alpha", 2), row("zeta", 2), row("mid", 1)]
        );
        assert!(full.modules.iter().all(|row| row.name != OTHER));
        assert_eq!(sum(&full.modules), 5);
        assert_eq!(sum(&full.types), 5);

        let top = breakdown(&verdicts, Some(1));
        assert_eq!(top.modules, vec![row("module.a", 2), row(OTHER, 3)]);
        assert_eq!(top.types, vec![row("alpha", 2), row(OTHER, 3)]);
        assert_eq!(sum(&top.modules), 5);
        assert_eq!(sum(&top.types), 5);
    }

    #[test]
    fn non_counting_rows_are_dropped_and_blank_modules_are_root() {
        let mut skipped = counting(Some("module.skip"), "null_resource");
        skipped.counts = false;
        skipped.rule = Rule::R3;
        skipped.reason = Rule::R3.reason();
        let verdicts = vec![
            counting(None, "random_id"),
            counting(Some(""), "random_id"),
            skipped,
        ];
        let report = breakdown(&verdicts, None);
        assert_eq!(report.modules, vec![row(ROOT_MODULE, 2)]);
        assert_eq!(report.types, vec![row("random_id", 2)]);
    }

    #[test]
    fn top_zero_collapses_to_other_and_a_wide_top_does_not() {
        let verdicts = vec![counting(Some("module.a"), "random_id")];
        assert_eq!(breakdown(&verdicts, Some(0)).modules, vec![row(OTHER, 1)]);
        assert_eq!(
            breakdown(&verdicts, Some(1)).modules,
            vec![row("module.a", 1)]
        );
        assert_eq!(
            breakdown(&verdicts, Some(u64::MAX)).modules,
            vec![row("module.a", 1)]
        );
        assert!(breakdown(&[] as &[Verdict], Some(0)).modules.is_empty());
        assert!(breakdown(&[] as &[Verdict], None).types.is_empty());
    }

    #[test]
    fn real_other_name_does_not_collide_with_remainder() {
        // Reported: module "other" with 5, module "zzz" with 1, --top 1 →
        // two rows both named "other". Remainder must become "(other)".
        let mut verdicts = Vec::new();
        for _ in 0..5 {
            verdicts.push(counting(Some("other"), "random_id"));
        }
        verdicts.push(counting(Some("zzz"), "random_id"));
        let top = breakdown(&verdicts, Some(1));
        assert_eq!(
            top.modules,
            vec![row("other", 5), row("(other)", 1)],
            "module list must keep unique names"
        );
        let names: Vec<_> = top.modules.iter().map(|r| r.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            names.len(),
            sorted.len(),
            "duplicate module names: {names:?}"
        );

        // Same for a type literally named "other".
        let mut types = Vec::new();
        for _ in 0..5 {
            types.push(counting(None, "other"));
        }
        types.push(counting(None, "zzz"));
        let top = breakdown(&types, Some(1));
        assert_eq!(top.types, vec![row("other", 5), row("(other)", 1)]);
        let names: Vec<_> = top.types.iter().map(|r| r.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len(), "duplicate type names: {names:?}");
    }

    #[test]
    fn remainder_skips_other_and_paren_other_when_both_are_real() {
        let verdicts = vec![
            counting(Some("other"), "random_id"),
            counting(Some("other"), "random_id"),
            counting(Some("(other)"), "random_id"),
            counting(Some("zzz"), "random_id"),
        ];
        let top = breakdown(&verdicts, Some(2));
        // top two by count: "other" (2), then "(other)" and "zzz" tied at 1 —
        // alphabetical: "(other)" before "zzz", so kept = other, (other);
        // remainder is zzz → needs "(other 2)".
        assert_eq!(
            top.modules,
            vec![row("other", 2), row("(other)", 1), row("(other 2)", 1)]
        );
    }
}
