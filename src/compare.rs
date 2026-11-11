//! Difference between a saved JSON report and the report of this run.
//!
//! Rows are matched by their text: a module path, a type name, a tier name
//! or an address. A path that is in both reports keeps that text. A change
//! of rank does not give it a new number, and does not list it as removed
//! and added. The printed difference is not a threshold. Exit 1 stays the
//! live `--max-rum` or `--max-increase` check.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::cost::TierCost;
use crate::diff::StateDiff;
use crate::render::CountedFile;
use crate::report::{self, Row};
use crate::rum::{self, total_rum};

const ESTIMATE: &str = "HCP Terraform bills the hourly peak; this is a single snapshot";

/// Why a saved report was refused. The text is the path and the kind of
/// error. It never includes bytes from the file.
#[derive(Debug)]
pub enum CompareError {
    /// The path could not be opened or read.
    Unreadable {
        /// Path that was refused.
        path: PathBuf,
        /// Short I/O kind, such as `not found`.
        kind: &'static str,
    },
    /// The path exists and is not a regular file. Nothing was read.
    NotRegular {
        /// Path that was refused.
        path: PathBuf,
    },
    /// More than 128 MiB were claimed. Nothing was parsed.
    TooLarge {
        /// Path that was refused.
        path: PathBuf,
    },
    /// UTF-16 or UTF-32. Nothing was decoded.
    NotUtf8 {
        /// Path that was refused.
        path: PathBuf,
        /// `UTF-16` or `UTF-32`.
        encoding: &'static str,
    },
    /// The file is not valid JSON.
    InvalidJson {
        /// Path that was refused.
        path: PathBuf,
        /// 1-based line reported by the JSON parser.
        line: usize,
        /// 1-based column reported by the JSON parser.
        column: usize,
    },
    /// Valid JSON that is not a rum-meter report of schema 1.
    NotReport {
        /// Path that was refused.
        path: PathBuf,
    },
    /// A count report was given to `diff`, or a diff report to `count`.
    WrongKind {
        /// Path that was refused.
        path: PathBuf,
        /// `count report` or `diff report`.
        found: &'static str,
        /// `count report` or `diff report`.
        expected: &'static str,
    },
    /// The same module path, type name, tier name or address occurs twice.
    Repeated {
        /// Path that was refused.
        path: PathBuf,
        /// `module path`, `type name`, `tier name` or `address`.
        what: &'static str,
    },
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable { path, kind } => write!(
                f,
                "rum-meter: {}: unreadable ({kind}); check that the path exists and that you may read it",
                esc_path(path)
            ),
            Self::NotRegular { path } => write!(
                f,
                "rum-meter: {}: not a regular file; pass a regular file",
                esc_path(path)
            ),
            Self::TooLarge { path } => write!(
                f,
                "rum-meter: {}: file is larger than {} MiB; pass a smaller file",
                esc_path(path),
                crate::regular::MAX_INPUT_BYTES / (1024 * 1024)
            ),
            Self::NotUtf8 { path, encoding } => write!(
                f,
                "rum-meter: {}: file is {encoding}; save it as UTF-8",
                esc_path(path)
            ),
            Self::InvalidJson { path, line, column } => write!(
                f,
                "rum-meter: {}: invalid json at line {line} column {column}; fix the file there, or write it again with --json",
                esc_path(path)
            ),
            Self::NotReport { path } => write!(
                f,
                "rum-meter: {}: file is not a rum-meter json report; pass a report written with --json",
                esc_path(path)
            ),
            Self::WrongKind {
                path,
                found,
                expected,
            } => write!(
                f,
                "rum-meter: {}: report is a {found}; pass a {expected} written with --json",
                esc_path(path)
            ),
            Self::Repeated { path, what } => write!(
                f,
                "rum-meter: {}: report repeats a {what}; write the report again with --json",
                esc_path(path)
            ),
        }
    }
}

impl std::error::Error for CompareError {}

impl CompareError {
    /// Next step, shown only with extra context (`--verbose` or a terminal).
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Unreadable { .. } => "check that the path exists and that you may read it",
            Self::NotRegular { .. } => "pass a regular file",
            Self::TooLarge { .. } => "pass a smaller file",
            Self::NotUtf8 { .. } => "save the file as UTF-8",
            Self::InvalidJson { .. } => "write the report again with --json",
            Self::NotReport { .. } => "pass a report written with --json",
            Self::WrongKind { expected, .. } => {
                if *expected == "count report" {
                    "pass a count report written with --json"
                } else {
                    "pass a diff report written with --json"
                }
            }
            Self::Repeated { .. } => "write the report again with --json",
        }
    }
}

struct SavedCount {
    total_rum: u64,
    modules: BTreeMap<String, u64>,
    types: BTreeMap<String, u64>,
    costs: BTreeMap<String, (String, String)>,
}

struct SavedDiff {
    old_rum: u64,
    new_rum: u64,
    rum_delta: i64,
    modules: BTreeMap<String, u64>,
    types: BTreeMap<String, u64>,
    costs: BTreeMap<String, (String, String)>,
    added: BTreeSet<String>,
    removed: BTreeSet<String>,
    changed_rule: BTreeMap<String, (String, String)>,
}

enum Saved {
    Count(SavedCount),
    Diff(SavedDiff),
}

/// Text difference between `saved_path` and this `count` run.
///
/// The string ends with a newline. Paths are matched by name.
pub fn render_count_difference(
    saved_path: &Path,
    files: &[CountedFile<'_>],
    top: Option<u64>,
    costs: &[TierCost],
) -> Result<String, CompareError> {
    let saved = load(saved_path)?;
    let Saved::Count(saved) = saved else {
        return Err(CompareError::WrongKind {
            path: saved_path.to_path_buf(),
            found: "diff report",
            expected: "count report",
        });
    };
    let total = files.iter().map(|file| total_rum(file.verdicts)).sum();
    let groups = report::breakdown(files.iter().flat_map(|file| file.verdicts), top);
    let mut out = String::new();
    write_compared(&mut out, saved_path);
    let live = as_u64(total);
    writeln!(out, "total rum: {} -> {live}", saved.total_rum).expect("write to String");
    writeln!(
        out,
        "rum delta: {}",
        signed(count_delta(saved.total_rum, live))
    )
    .expect("write to String");
    write_counts(
        &mut out,
        "modules",
        &saved.modules,
        &rows_map(&groups.modules),
    );
    write_counts(&mut out, "types", &saved.types, &rows_map(&groups.types));
    write_money(&mut out, &saved.costs, &money_map(costs));
    writeln!(out, "estimate: {ESTIMATE}").expect("write to String");
    Ok(out)
}

/// Text difference between `saved_path` and this `diff` run.
///
/// The string ends with a newline. Addresses are matched by their full text.
pub fn render_diff_difference(
    saved_path: &Path,
    diff: &StateDiff,
    new_verdicts: &[rum::Verdict],
    top: Option<u64>,
    costs: &[TierCost],
) -> Result<String, CompareError> {
    let saved = load(saved_path)?;
    let Saved::Diff(saved) = saved else {
        return Err(CompareError::WrongKind {
            path: saved_path.to_path_buf(),
            found: "count report",
            expected: "diff report",
        });
    };
    let groups = report::breakdown(new_verdicts, top);
    let mut out = String::new();
    write_compared(&mut out, saved_path);
    writeln!(
        out,
        "old rum: {} -> {}",
        saved.old_rum,
        as_u64(diff.old_rum)
    )
    .expect("write to String");
    writeln!(
        out,
        "new rum: {} -> {}",
        saved.new_rum,
        as_u64(diff.new_rum)
    )
    .expect("write to String");
    writeln!(
        out,
        "rum delta: {} -> {}",
        signed(saved.rum_delta),
        signed(diff.rum_delta())
    )
    .expect("write to String");
    write_counts(
        &mut out,
        "modules",
        &saved.modules,
        &rows_map(&groups.modules),
    );
    write_counts(&mut out, "types", &saved.types, &rows_map(&groups.types));
    write_money(&mut out, &saved.costs, &money_map(costs));
    write_addresses(&mut out, "added", &saved.added, &address_set(&diff.added));
    write_addresses(
        &mut out,
        "removed",
        &saved.removed,
        &address_set(&diff.removed),
    );
    write_rules(&mut out, &saved.changed_rule, &rule_map(diff));
    writeln!(out, "estimate: {ESTIMATE}").expect("write to String");
    Ok(out)
}

fn load(path: &Path) -> Result<Saved, CompareError> {
    let bytes = crate::regular::read_regular(path).map_err(|err| match err {
        crate::regular::RegularError::NotRegular => CompareError::NotRegular {
            path: path.to_path_buf(),
        },
        crate::regular::RegularError::TooLarge => CompareError::TooLarge {
            path: path.to_path_buf(),
        },
        crate::regular::RegularError::Unreadable { kind } => CompareError::Unreadable {
            path: path.to_path_buf(),
            kind,
        },
    })?;
    let body = crate::encoding::utf8_body(&bytes).map_err(|wide| CompareError::NotUtf8 {
        path: path.to_path_buf(),
        encoding: wide.name(),
    })?;
    let value: Value = serde_json::from_slice(body).map_err(|err| CompareError::InvalidJson {
        path: path.to_path_buf(),
        line: err.line(),
        column: err.column(),
    })?;
    let Some(obj) = value.as_object() else {
        return Err(not_report(path));
    };
    if obj.get("schema_version").and_then(Value::as_u64) != Some(1)
        || obj.get("tool").and_then(Value::as_str) != Some("rum-meter")
    {
        return Err(not_report(path));
    }
    match obj.get("total_rum") {
        Some(Value::Number(_)) => Ok(Saved::Count(parse_count(path, obj)?)),
        Some(Value::Object(_)) => Ok(Saved::Diff(parse_diff(path, obj)?)),
        _ => Err(not_report(path)),
    }
}

fn parse_count(
    path: &Path,
    obj: &serde_json::Map<String, Value>,
) -> Result<SavedCount, CompareError> {
    let total_rum = obj
        .get("total_rum")
        .and_then(Value::as_u64)
        .ok_or_else(|| not_report(path))?;
    Ok(SavedCount {
        total_rum,
        modules: parse_rows(path, obj.get("by_module"), "module path")?,
        types: parse_rows(path, obj.get("by_type"), "type name")?,
        costs: parse_costs(path, obj.get("costs"))?,
    })
}

fn parse_diff(
    path: &Path,
    obj: &serde_json::Map<String, Value>,
) -> Result<SavedDiff, CompareError> {
    let totals = obj
        .get("total_rum")
        .and_then(Value::as_object)
        .ok_or_else(|| not_report(path))?;
    let old_rum = totals
        .get("old")
        .and_then(Value::as_u64)
        .ok_or_else(|| not_report(path))?;
    let new_rum = totals
        .get("new")
        .and_then(Value::as_u64)
        .ok_or_else(|| not_report(path))?;
    let rum_delta = obj
        .get("rum_delta")
        .and_then(Value::as_i64)
        .ok_or_else(|| not_report(path))?;
    Ok(SavedDiff {
        old_rum,
        new_rum,
        rum_delta,
        modules: parse_rows(path, obj.get("by_module"), "module path")?,
        types: parse_rows(path, obj.get("by_type"), "type name")?,
        costs: parse_costs(path, obj.get("costs"))?,
        added: parse_addresses(path, obj.get("added"))?,
        removed: parse_addresses(path, obj.get("removed"))?,
        changed_rule: parse_rules(path, obj.get("changed_rule"))?,
    })
}

fn parse_rows(
    path: &Path,
    value: Option<&Value>,
    what: &'static str,
) -> Result<BTreeMap<String, u64>, CompareError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(not_report(path));
    };
    let mut map = BTreeMap::new();
    for item in items {
        let Some(row) = item.as_object() else {
            return Err(not_report(path));
        };
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            return Err(not_report(path));
        };
        let Some(count) = row.get("count").and_then(Value::as_u64) else {
            return Err(not_report(path));
        };
        if map.insert(name.to_string(), count).is_some() {
            return Err(repeated(path, what));
        }
    }
    Ok(map)
}

fn parse_costs(
    path: &Path,
    value: Option<&Value>,
) -> Result<BTreeMap<String, (String, String)>, CompareError> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let Some(items) = value.as_array() else {
        return Err(not_report(path));
    };
    let mut map = BTreeMap::new();
    for item in items {
        let Some(row) = item.as_object() else {
            return Err(not_report(path));
        };
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            return Err(not_report(path));
        };
        let Some(currency) = row.get("currency").and_then(Value::as_str) else {
            return Err(not_report(path));
        };
        let Some(amount) = row.get("amount").and_then(Value::as_str) else {
            return Err(not_report(path));
        };
        if map
            .insert(name.to_string(), (currency.to_string(), amount.to_string()))
            .is_some()
        {
            return Err(repeated(path, "tier name"));
        }
    }
    Ok(map)
}

fn parse_addresses(path: &Path, value: Option<&Value>) -> Result<BTreeSet<String>, CompareError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(not_report(path));
    };
    let mut set = BTreeSet::new();
    for item in items {
        let Some(address) = item.as_str() else {
            return Err(not_report(path));
        };
        if !set.insert(address.to_string()) {
            return Err(repeated(path, "address"));
        }
    }
    Ok(set)
}

fn parse_rules(
    path: &Path,
    value: Option<&Value>,
) -> Result<BTreeMap<String, (String, String)>, CompareError> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Err(not_report(path));
    };
    let mut map = BTreeMap::new();
    for item in items {
        let Some(row) = item.as_object() else {
            return Err(not_report(path));
        };
        let Some(address) = row.get("address").and_then(Value::as_str) else {
            return Err(not_report(path));
        };
        let Some(old_rule) = row.get("old_rule").and_then(rule_code) else {
            return Err(not_report(path));
        };
        let Some(new_rule) = row.get("new_rule").and_then(rule_code) else {
            return Err(not_report(path));
        };
        if map
            .insert(
                address.to_string(),
                (old_rule.to_string(), new_rule.to_string()),
            )
            .is_some()
        {
            return Err(repeated(path, "address"));
        }
    }
    Ok(map)
}

fn rule_code(value: &Value) -> Option<&'static str> {
    match value.as_str()? {
        "R1" => Some("R1"),
        "R2" => Some("R2"),
        "R3" => Some("R3"),
        "R4" => Some("R4"),
        _ => None,
    }
}

fn not_report(path: &Path) -> CompareError {
    CompareError::NotReport {
        path: path.to_path_buf(),
    }
}

fn repeated(path: &Path, what: &'static str) -> CompareError {
    CompareError::Repeated {
        path: path.to_path_buf(),
        what,
    }
}

fn rows_map(rows: &[Row]) -> BTreeMap<String, u64> {
    rows.iter()
        .map(|row| (row.name.clone(), as_u64(row.count)))
        .collect()
}

fn money_map(costs: &[TierCost]) -> BTreeMap<String, (String, String)> {
    costs
        .iter()
        .map(|cost| {
            (
                cost.name.clone(),
                (cost.currency.clone(), cost.amount.clone()),
            )
        })
        .collect()
}

fn address_set(addresses: &[String]) -> BTreeSet<String> {
    addresses.iter().cloned().collect()
}

fn rule_map(diff: &StateDiff) -> BTreeMap<String, (String, String)> {
    diff.changed_rule
        .iter()
        .map(|change| {
            (
                change.address.clone(),
                (
                    change.old_rule.code().to_string(),
                    change.new_rule.code().to_string(),
                ),
            )
        })
        .collect()
}

fn as_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn count_delta(old: u64, new: u64) -> i64 {
    let old = i64::try_from(old).unwrap_or(i64::MAX);
    let new = i64::try_from(new).unwrap_or(i64::MAX);
    new.saturating_sub(old)
}

fn signed(delta: i64) -> String {
    if delta > 0 {
        format!("+{delta}")
    } else {
        delta.to_string()
    }
}

fn esc(text: &str) -> std::borrow::Cow<'_, str> {
    rum::escape_controls(text)
}

fn esc_path(path: &Path) -> String {
    esc(&path.display().to_string()).into_owned()
}

fn write_compared(out: &mut String, path: &Path) {
    writeln!(out, "compared: {}", esc_path(path)).expect("write to String");
}

/// Changed, added and removed rows, each in name order.
///
/// A name that is in both maps stays that name. It is not paired by position.
struct NamedDelta<T> {
    changed: Vec<(String, T, T)>,
    added: Vec<(String, T)>,
    removed: Vec<(String, T)>,
}

fn named_delta<T: Clone + PartialEq>(
    saved: &BTreeMap<String, T>,
    live: &BTreeMap<String, T>,
) -> NamedDelta<T> {
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let names: BTreeSet<&str> = saved
        .keys()
        .map(String::as_str)
        .chain(live.keys().map(String::as_str))
        .collect();
    for name in names {
        match (saved.get(name), live.get(name)) {
            (Some(old), Some(new)) if old != new => {
                changed.push((name.to_string(), old.clone(), new.clone()));
            }
            (Some(_), Some(_)) => {}
            (None, Some(new)) => added.push((name.to_string(), new.clone())),
            (Some(old), None) => removed.push((name.to_string(), old.clone())),
            (None, None) => {}
        }
    }
    NamedDelta {
        changed,
        added,
        removed,
    }
}

fn write_counts(
    out: &mut String,
    label: &str,
    saved: &BTreeMap<String, u64>,
    live: &BTreeMap<String, u64>,
) {
    let NamedDelta {
        changed,
        added,
        removed,
    } = named_delta(saved, live);
    writeln!(out, "{label}:").expect("write to String");
    for (name, old, new) in changed {
        writeln!(out, "  {}: {old} -> {new}", esc(&name)).expect("write to String");
    }
    writeln!(out, "{label} added:").expect("write to String");
    for (name, count) in added {
        writeln!(out, "  {}: {count}", esc(&name)).expect("write to String");
    }
    writeln!(out, "{label} removed:").expect("write to String");
    for (name, count) in removed {
        writeln!(out, "  {}: {count}", esc(&name)).expect("write to String");
    }
}

fn write_money(
    out: &mut String,
    saved: &BTreeMap<String, (String, String)>,
    live: &BTreeMap<String, (String, String)>,
) {
    if saved.is_empty() && live.is_empty() {
        return;
    }
    let NamedDelta {
        changed,
        added,
        removed,
    } = named_delta(saved, live);
    writeln!(out, "costs:").expect("write to String");
    for (name, old, new) in changed {
        writeln!(
            out,
            "  {}: {} {} -> {} {}",
            esc(&name),
            esc(&old.0),
            esc(&old.1),
            esc(&new.0),
            esc(&new.1)
        )
        .expect("write to String");
    }
    writeln!(out, "costs added:").expect("write to String");
    for (name, (currency, amount)) in added {
        writeln!(out, "  {}: {} {}", esc(&name), esc(&currency), esc(&amount))
            .expect("write to String");
    }
    writeln!(out, "costs removed:").expect("write to String");
    for (name, (currency, amount)) in removed {
        writeln!(out, "  {}: {} {}", esc(&name), esc(&currency), esc(&amount))
            .expect("write to String");
    }
}

/// Addresses present on only one side, sorted by the address text.
///
/// An address that is in both sets is left out. It is not paired with the
/// address that happens to sit at the same index.
fn address_marks<'a>(
    saved: &'a BTreeSet<String>,
    live: &'a BTreeSet<String>,
) -> Vec<(&'a str, char)> {
    let mut rows = Vec::new();
    for address in live {
        if !saved.contains(address) {
            rows.push((address.as_str(), '+'));
        }
    }
    for address in saved {
        if !live.contains(address) {
            rows.push((address.as_str(), '-'));
        }
    }
    rows.sort_by(|left, right| left.0.cmp(right.0));
    rows
}

fn write_addresses(
    out: &mut String,
    label: &str,
    saved: &BTreeSet<String>,
    live: &BTreeSet<String>,
) {
    writeln!(out, "{label}:").expect("write to String");
    for (address, sign) in address_marks(saved, live) {
        writeln!(out, "  {sign} {}", esc(address)).expect("write to String");
    }
}

fn write_rules(
    out: &mut String,
    saved: &BTreeMap<String, (String, String)>,
    live: &BTreeMap<String, (String, String)>,
) {
    writeln!(out, "changed rule:").expect("write to String");
    let NamedDelta {
        changed,
        added,
        removed,
    } = named_delta(saved, live);
    let mut rows = Vec::new();
    for (address, old, new) in changed {
        rows.push((
            address.clone(),
            format!(
                "  {}: {} -> {} to {} -> {}",
                esc(&address),
                old.0,
                old.1,
                new.0,
                new.1
            ),
        ));
    }
    for (address, (old_rule, new_rule)) in added {
        rows.push((
            address.clone(),
            format!("  + {}: {old_rule} -> {new_rule}", esc(&address)),
        ));
    }
    for (address, (old_rule, new_rule)) in removed {
        rows.push((
            address.clone(),
            format!("  - {}: {old_rule} -> {new_rule}", esc(&address)),
        ));
    }
    rows.sort_by(|left, right| left.0.cmp(&right.0));
    for (_, line) in rows {
        writeln!(out, "{line}").expect("write to String");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(rows: &[(&str, u64)]) -> BTreeMap<String, u64> {
        rows.iter()
            .map(|(name, count)| ((*name).to_string(), *count))
            .collect()
    }

    fn set(rows: &[&str]) -> BTreeSet<String> {
        rows.iter().map(|row| (*row).to_string()).collect()
    }

    #[test]
    fn a_rank_change_keeps_the_path_and_does_not_renumber_it() {
        // Saved rank is module.a then module.b. Live rank is the reverse.
        // The path stays the path. It is not dropped and added under a new index.
        let saved = map(&[("module.a", 5), ("module.b", 1)]);
        let live = map(&[("module.b", 5), ("module.a", 1)]);
        let NamedDelta {
            changed,
            added,
            removed,
        } = named_delta(&saved, &live);
        assert!(added.is_empty());
        assert!(removed.is_empty());
        assert_eq!(
            changed,
            vec![
                ("module.a".to_string(), 5, 1),
                ("module.b".to_string(), 1, 5),
            ]
        );
    }

    #[test]
    fn the_same_counts_in_another_order_are_not_a_difference() {
        let saved = map(&[("module.b", 5), ("module.a", 1), ("other", 2)]);
        let live = map(&[("other", 2), ("module.a", 1), ("module.b", 5)]);
        let NamedDelta {
            changed,
            added,
            removed,
        } = named_delta(&saved, &live);
        assert!(changed.is_empty());
        assert!(added.is_empty());
        assert!(removed.is_empty());
    }

    #[test]
    fn an_address_in_both_lists_keeps_its_index() {
        let saved = set(&["random_id.kept[1]", "random_id.zzz[0]"]);
        let live = set(&["random_id.extra[0]", "random_id.kept[1]"]);
        let marks = address_marks(&saved, &live);
        assert_eq!(
            marks,
            vec![("random_id.extra[0]", '+'), ("random_id.zzz[0]", '-')]
        );
        assert!(
            marks
                .iter()
                .all(|(address, _)| *address != "random_id.kept[1]")
        );
        assert!(
            marks
                .iter()
                .all(|(address, _)| *address != "random_id.kept[0]")
        );
        assert!(
            marks
                .iter()
                .all(|(address, _)| *address != "random_id.kept[2]")
        );
    }
}
