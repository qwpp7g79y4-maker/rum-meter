//! Text reports for `count` and `diff`.
//!
//! Each state file is one block (`file`, `rum`, `not counted`), then a sum
//! (`total rum`, `not counted`), the billable breakdown (`modules`, `types`),
//! one line per priced tier (`costs`) and one estimate line. With `explain`,
//! each verdict is one indented line under its file: address, `counts` or
//! `does not count`, and the rule code. A tainted object appends ` tainted`.
//! Non-counting rules are always R2, R3 and R4, in that order.
//!
//! `diff` prints the old and new paths, both RUM totals, the RUM delta, the
//! added and removed counting addresses, rule changes, the per-tier cost delta
//! when prices were given, and the same estimate line.
//!
//! `--json` prints one object with `schema_version` 1 instead of that text.
//! `instances` is present only with `explain`. An instance object gains
//! `deposed` (the key) and `tainted` (`true`) only when they apply.
//! `costs` is present only when tier costs were passed. On a diff,
//! `cost_delta` is present only when cost deltas were passed. The object
//! carries paths, addresses, modules, types, rule codes and amounts, never
//! attribute values. Text escapes control characters and invisible or
//! bidirectional format characters in paths, price labels, addresses, module
//! names and types (`\uXXXX`). JSON keeps those characters unchanged.

use std::fmt::Write;
use std::path::Path;

use serde::Serialize;

use crate::cost::TierCost;
use crate::diff::{CostDelta, StateDiff};
use crate::report::{self, Row};
use crate::rum::{self, Rule, Verdict, total_rum};

const ESTIMATE: &str = "HCP Terraform bills the hourly peak; this is a single snapshot";
const SCHEMA_VERSION: u64 = 1;
const TOOL: &str = "rum-meter";

/// One loaded state file and the verdicts for its instances.
pub struct CountedFile<'a> {
    pub path: &'a Path,
    pub verdicts: &'a [Verdict],
}

/// Render the human-readable `count` report. The string ends with a newline.
///
/// `top` caps the module list and the type list. `None` lists every row.
/// `costs` is empty when no price file was given; then the report has no cost lines.
pub fn render_count_text(
    files: &[CountedFile<'_>],
    explain: bool,
    top: Option<u64>,
    costs: &[TierCost],
) -> String {
    let mut out = String::new();
    let mut not_counted = NotCounted::default();
    for file in files {
        let file_not_counted = NotCounted::from_verdicts(file.verdicts);
        not_counted.add(file_not_counted);
        writeln!(
            out,
            "file: {}",
            rum::escape_controls(&file.path.display().to_string())
        )
        .expect("write to String");
        writeln!(out, "rum: {}", total_rum(file.verdicts)).expect("write to String");
        writeln!(out, "not counted: {file_not_counted}").expect("write to String");
        if explain {
            for verdict in file.verdicts {
                writeln!(
                    out,
                    "  {} {} {}{}",
                    rum::escape_controls(&verdict.address),
                    count_label(verdict.counts),
                    verdict.rule.code(),
                    tainted_mark(verdict.tainted),
                )
                .expect("write to String");
            }
        }
    }
    let total: usize = files.iter().map(|file| total_rum(file.verdicts)).sum();
    writeln!(out, "total rum: {total}").expect("write to String");
    writeln!(out, "not counted: {not_counted}").expect("write to String");
    let groups = report::breakdown(files.iter().flat_map(|file| file.verdicts), top);
    write_group(&mut out, "modules", &groups.modules);
    write_group(&mut out, "types", &groups.types);
    write_costs(&mut out, costs);
    writeln!(out, "estimate: {ESTIMATE}").expect("write to String");
    out
}

/// Render the human-readable `diff` report. The string ends with a newline.
///
/// `costs` is empty when no price file was given; then the report has no cost lines.
pub fn render_diff_text(old: &Path, new: &Path, diff: &StateDiff, costs: &[CostDelta]) -> String {
    let mut out = String::new();
    let delta = format_signed_int(diff.rum_delta());
    writeln!(
        out,
        "old: {}",
        rum::escape_controls(&old.display().to_string())
    )
    .expect("write to String");
    writeln!(
        out,
        "new: {}",
        rum::escape_controls(&new.display().to_string())
    )
    .expect("write to String");
    writeln!(out, "rum: {} -> {}", diff.old_rum, diff.new_rum).expect("write to String");
    writeln!(out, "rum delta: {delta}").expect("write to String");
    write_addresses(&mut out, "added", &diff.added);
    write_addresses(&mut out, "removed", &diff.removed);
    writeln!(out, "changed rule:").expect("write to String");
    for change in &diff.changed_rule {
        writeln!(
            out,
            "  {} {} -> {}",
            rum::escape_controls(&change.address),
            change.old_rule.code(),
            change.new_rule.code()
        )
        .expect("write to String");
    }
    if !costs.is_empty() {
        writeln!(out, "cost delta:").expect("write to String");
        for cost in costs {
            writeln!(
                out,
                "  {}: {} {}",
                rum::escape_controls(&cost.name),
                rum::escape_controls(&cost.currency),
                cost.amount
            )
            .expect("write to String");
        }
    }
    writeln!(out, "estimate: {ESTIMATE}").expect("write to String");
    out
}

fn format_signed_int(delta: i64) -> String {
    if delta > 0 {
        format!("+{delta}")
    } else {
        delta.to_string()
    }
}

fn write_addresses(out: &mut String, label: &str, addresses: &[String]) {
    writeln!(out, "{label}:").expect("write to String");
    for address in addresses {
        writeln!(out, "  {}", rum::escape_controls(address)).expect("write to String");
    }
}

fn write_costs(out: &mut String, costs: &[TierCost]) {
    if costs.is_empty() {
        return;
    }
    writeln!(out, "costs:").expect("write to String");
    for cost in costs {
        writeln!(
            out,
            "  {}: {} {}",
            rum::escape_controls(&cost.name),
            rum::escape_controls(&cost.currency),
            cost.amount
        )
        .expect("write to String");
    }
}

fn write_group(out: &mut String, label: &str, rows: &[Row]) {
    writeln!(out, "{label}:").expect("write to String");
    for row in rows {
        writeln!(out, "  {}: {}", rum::escape_controls(&row.name), row.count)
            .expect("write to String");
    }
}

#[derive(Clone, Copy, Default)]
struct NotCounted {
    r2: usize,
    r3: usize,
    r4: usize,
}

impl NotCounted {
    fn from_verdicts(verdicts: &[Verdict]) -> Self {
        let mut counts = Self::default();
        for verdict in verdicts {
            match verdict.rule {
                Rule::R1 => {}
                Rule::R2 => counts.r2 += 1,
                Rule::R3 => counts.r3 += 1,
                Rule::R4 => counts.r4 += 1,
            }
        }
        counts
    }

    fn add(&mut self, other: Self) {
        self.r2 += other.r2;
        self.r3 += other.r3;
        self.r4 += other.r4;
    }
}

impl std::fmt::Display for NotCounted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "R2 {}, R3 {}, R4 {}", self.r2, self.r3, self.r4)
    }
}

fn count_label(counts: bool) -> &'static str {
    if counts { "counts" } else { "does not count" }
}

fn tainted_mark(tainted: bool) -> &'static str {
    if tainted { " tainted" } else { "" }
}

/// Paths, verdicts and priced totals for one `diff` JSON report.
///
/// `costs` is the new state's monthly estimate. Both `costs` and `cost_deltas`
/// are empty when no price file was given.
pub struct DiffJsonInput<'a> {
    pub old: &'a Path,
    pub new: &'a Path,
    pub old_verdicts: &'a [Verdict],
    pub new_verdicts: &'a [Verdict],
    pub diff: &'a StateDiff,
    pub costs: &'a [TierCost],
    pub cost_deltas: &'a [CostDelta],
}

/// Render the `count` report as one JSON object. The string ends with a newline.
///
/// `instances` is omitted unless `explain` is set. `costs` is omitted when empty.
pub fn render_count_json(
    files: &[CountedFile<'_>],
    explain: bool,
    top: Option<u64>,
    costs: &[TierCost],
) -> String {
    let groups = report::breakdown(files.iter().flat_map(|file| file.verdicts), top);
    let total_rum = files.iter().map(|file| total_rum(file.verdicts)).sum();
    let report = CountJson {
        schema_version: SCHEMA_VERSION,
        tool: TOOL,
        estimate: ESTIMATE,
        inputs: files
            .iter()
            .map(|file| file.path.display().to_string())
            .collect(),
        total_rum,
        by_module: rows_json(&groups.modules),
        by_type: rows_json(&groups.types),
        instances: explain.then(|| count_instances(files)),
        costs: costs_json(costs),
    };
    write_json(&report)
}

/// Render the `diff` report as one JSON object. The string ends with a newline.
///
/// `by_module` and `by_type` describe the new state. `instances` is omitted
/// unless `explain` is set. `costs` and `cost_delta` are omitted when empty.
pub fn render_diff_json(input: &DiffJsonInput<'_>, explain: bool, top: Option<u64>) -> String {
    let groups = report::breakdown(input.new_verdicts, top);
    let mut instances = Vec::new();
    if explain {
        instances.extend(instances_from(input.old, input.old_verdicts));
        instances.extend(instances_from(input.new, input.new_verdicts));
    }
    let report = DiffJson {
        schema_version: SCHEMA_VERSION,
        tool: TOOL,
        estimate: ESTIMATE,
        inputs: DiffInputs {
            old: input.old.display().to_string(),
            new: input.new.display().to_string(),
        },
        total_rum: RumSides {
            old: input.diff.old_rum,
            new: input.diff.new_rum,
        },
        by_module: rows_json(&groups.modules),
        by_type: rows_json(&groups.types),
        instances: explain.then_some(instances),
        costs: costs_json(input.costs),
        added: input.diff.added.clone(),
        removed: input.diff.removed.clone(),
        changed_rule: input
            .diff
            .changed_rule
            .iter()
            .map(|change| RuleChangeJson {
                address: change.address.clone(),
                old_rule: change.old_rule.code(),
                new_rule: change.new_rule.code(),
            })
            .collect(),
        rum_delta: input.diff.rum_delta(),
        cost_delta: deltas_json(input.cost_deltas),
    };
    write_json(&report)
}

#[derive(Serialize)]
struct CountJson {
    schema_version: u64,
    tool: &'static str,
    estimate: &'static str,
    inputs: Vec<String>,
    total_rum: usize,
    by_module: Vec<RowJson>,
    by_type: Vec<RowJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instances: Option<Vec<InstanceJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    costs: Option<Vec<CostJson>>,
}

#[derive(Serialize)]
struct DiffJson {
    schema_version: u64,
    tool: &'static str,
    estimate: &'static str,
    inputs: DiffInputs,
    total_rum: RumSides,
    by_module: Vec<RowJson>,
    by_type: Vec<RowJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instances: Option<Vec<InstanceJson>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    costs: Option<Vec<CostJson>>,
    added: Vec<String>,
    removed: Vec<String>,
    changed_rule: Vec<RuleChangeJson>,
    rum_delta: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_delta: Option<Vec<CostJson>>,
}

#[derive(Serialize)]
struct DiffInputs {
    old: String,
    new: String,
}

#[derive(Serialize)]
struct RumSides {
    old: usize,
    new: usize,
}

#[derive(Serialize)]
struct RowJson {
    name: String,
    count: usize,
}

#[derive(Serialize)]
struct InstanceJson {
    file: String,
    address: String,
    counts: bool,
    rule: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    deposed: Option<String>,
    #[serde(skip_serializing_if = "not_tainted")]
    tainted: bool,
}

fn not_tainted(tainted: &bool) -> bool {
    !*tainted
}

#[derive(Serialize)]
struct CostJson {
    name: String,
    currency: String,
    amount: String,
}

#[derive(Serialize)]
struct RuleChangeJson {
    address: String,
    old_rule: &'static str,
    new_rule: &'static str,
}

fn write_json(value: &impl Serialize) -> String {
    let mut json = serde_json::to_string_pretty(value).expect("json report");
    json.push('\n');
    json
}

fn rows_json(rows: &[Row]) -> Vec<RowJson> {
    rows.iter()
        .map(|row| RowJson {
            name: row.name.clone(),
            count: row.count,
        })
        .collect()
}

fn costs_json(costs: &[TierCost]) -> Option<Vec<CostJson>> {
    if costs.is_empty() {
        return None;
    }
    Some(costs.iter().map(cost_json).collect())
}

fn deltas_json(deltas: &[CostDelta]) -> Option<Vec<CostJson>> {
    if deltas.is_empty() {
        return None;
    }
    Some(deltas.iter().map(delta_json).collect())
}

fn cost_json(cost: &TierCost) -> CostJson {
    CostJson {
        name: cost.name.clone(),
        currency: cost.currency.clone(),
        amount: cost.amount.clone(),
    }
}

fn delta_json(delta: &CostDelta) -> CostJson {
    CostJson {
        name: delta.name.clone(),
        currency: delta.currency.clone(),
        amount: delta.amount.clone(),
    }
}

fn count_instances(files: &[CountedFile<'_>]) -> Vec<InstanceJson> {
    let mut instances = Vec::new();
    for file in files {
        instances.extend(instances_from(file.path, file.verdicts));
    }
    instances
}

fn instances_from(path: &Path, verdicts: &[Verdict]) -> Vec<InstanceJson> {
    let file = path.display().to_string();
    verdicts
        .iter()
        .map(|verdict| InstanceJson {
            file: file.clone(),
            address: verdict.address.clone(),
            counts: verdict.counts,
            rule: verdict.rule.code(),
            deposed: verdict.deposed.clone(),
            tainted: verdict.tainted,
        })
        .collect()
}
