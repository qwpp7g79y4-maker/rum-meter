//! Markdown and HTML reports for `count` and `diff`.
//!
//! Both formats are built from one small block model, so they always carry
//! the same content as the text and JSON reports: paths, addresses, modules,
//! types, rule codes and amounts, never attribute values. Control and
//! invisible characters are escaped (`\uXXXX`) first, like the text report.
//! Markdown then backslash-escapes ASCII punctuation, and HTML escapes
//! `& < > " '`. The HTML page is self-contained: no scripts, no external
//! resources.

use std::fmt::Write;
use std::path::Path;

use crate::cost::TierCost;
use crate::diff::{CostDelta, StateDiff};
use crate::render::CountedFile;
use crate::report::{self, Row};
use crate::rum::{self, Rule, Verdict, total_rum};

const ESTIMATE: &str = "HCP Terraform bills the hourly peak; this is a single snapshot";

enum Block {
    Heading(&'static str),
    Para(String),
    Table {
        head: &'static [&'static str],
        rows: Vec<Vec<String>>,
    },
}

/// A document: title plus blocks. All strings are raw; formats escape them.
struct Doc {
    title: &'static str,
    blocks: Vec<Block>,
}

fn safe(text: &str) -> String {
    rum::escape_controls(text).into_owned()
}

fn path_text(path: &Path) -> String {
    safe(&path.display().to_string())
}

fn rule_counts<'a>(verdicts: impl IntoIterator<Item = &'a Verdict>) -> String {
    let (mut r2, mut r3, mut r4) = (0, 0, 0);
    for verdict in verdicts {
        match verdict.rule {
            Rule::R1 => {}
            Rule::R2 => r2 += 1,
            Rule::R3 => r3 += 1,
            Rule::R4 => r4 += 1,
        }
    }
    format!("R2 {r2}, R3 {r3}, R4 {r4}")
}

fn row_cells(rows: &[Row]) -> Vec<Vec<String>> {
    rows.iter()
        .map(|row| vec![safe(&row.name), row.count.to_string()])
        .collect()
}

fn cost_cells(costs: impl Iterator<Item = (String, String, String)>) -> Vec<Vec<String>> {
    costs
        .map(|(name, currency, amount)| vec![safe(&name), safe(&currency), amount])
        .collect()
}

fn address_cells(addresses: &[String]) -> Vec<Vec<String>> {
    addresses.iter().map(|a| vec![safe(a)]).collect()
}

fn instance_cells(path: &Path, verdicts: &[Verdict]) -> Vec<Vec<String>> {
    verdicts
        .iter()
        .map(|v| {
            vec![
                path_text(path),
                safe(&v.address),
                if v.counts { "counts" } else { "does not count" }.to_string(),
                v.rule.code().to_string(),
                if v.tainted { "yes" } else { "no" }.to_string(),
            ]
        })
        .collect()
}

const GROUP_HEAD: &[&str] = &["Name", "RUM"];
const COST_HEAD: &[&str] = &["Tier", "Currency", "Amount"];

fn push_table(
    doc: &mut Doc,
    heading: &'static str,
    head: &'static [&'static str],
    rows: Vec<Vec<String>>,
) {
    doc.blocks.push(Block::Heading(heading));
    if rows.is_empty() {
        doc.blocks.push(Block::Para("none".to_string()));
    } else {
        doc.blocks.push(Block::Table { head, rows });
    }
}

fn count_doc(
    files: &[CountedFile<'_>],
    explain: bool,
    top: Option<u64>,
    costs: &[TierCost],
) -> Doc {
    let mut doc = Doc {
        title: "rum-meter count",
        blocks: Vec::new(),
    };
    let total: usize = files.iter().map(|f| total_rum(f.verdicts)).sum();
    doc.blocks.push(Block::Para(format!("Total RUM: {total}")));
    doc.blocks.push(Block::Para(format!(
        "Not counted: {}",
        rule_counts(files.iter().flat_map(|f| f.verdicts.iter()))
    )));
    let file_rows = files
        .iter()
        .map(|f| {
            vec![
                path_text(f.path),
                total_rum(f.verdicts).to_string(),
                rule_counts(f.verdicts),
            ]
        })
        .collect();
    push_table(
        &mut doc,
        "Files",
        &["File", "RUM", "Not counted"],
        file_rows,
    );
    let groups = report::breakdown(files.iter().flat_map(|f| f.verdicts), top);
    push_table(&mut doc, "Modules", GROUP_HEAD, row_cells(&groups.modules));
    push_table(&mut doc, "Types", GROUP_HEAD, row_cells(&groups.types));
    if !costs.is_empty() {
        let rows = cost_cells(
            costs
                .iter()
                .map(|c| (c.name.clone(), c.currency.clone(), c.amount.clone())),
        );
        push_table(&mut doc, "Costs", COST_HEAD, rows);
    }
    if explain {
        let rows = files
            .iter()
            .flat_map(|f| instance_cells(f.path, f.verdicts))
            .collect();
        push_table(
            &mut doc,
            "Instances",
            &["File", "Address", "Verdict", "Rule", "Tainted"],
            rows,
        );
    }
    doc.blocks
        .push(Block::Para(format!("Estimate: {ESTIMATE}")));
    doc
}

fn diff_doc(old: &Path, new: &Path, diff: &StateDiff, costs: &[CostDelta]) -> Doc {
    let mut doc = Doc {
        title: "rum-meter diff",
        blocks: Vec::new(),
    };
    let delta = diff.rum_delta();
    let delta = if delta > 0 {
        format!("+{delta}")
    } else {
        delta.to_string()
    };
    doc.blocks
        .push(Block::Para(format!("Old: {}", path_text(old))));
    doc.blocks
        .push(Block::Para(format!("New: {}", path_text(new))));
    doc.blocks.push(Block::Para(format!(
        "RUM: {} -> {}",
        diff.old_rum, diff.new_rum
    )));
    doc.blocks.push(Block::Para(format!("RUM delta: {delta}")));
    push_table(&mut doc, "Added", &["Address"], address_cells(&diff.added));
    push_table(
        &mut doc,
        "Removed",
        &["Address"],
        address_cells(&diff.removed),
    );
    let changed = diff
        .changed_rule
        .iter()
        .map(|c| {
            vec![
                safe(&c.address),
                c.old_rule.code().to_string(),
                c.new_rule.code().to_string(),
            ]
        })
        .collect();
    push_table(
        &mut doc,
        "Changed rule",
        &["Address", "Old rule", "New rule"],
        changed,
    );
    if !costs.is_empty() {
        let rows = cost_cells(
            costs
                .iter()
                .map(|c| (c.name.clone(), c.currency.clone(), c.amount.clone())),
        );
        push_table(&mut doc, "Cost delta", COST_HEAD, rows);
    }
    doc.blocks
        .push(Block::Para(format!("Estimate: {ESTIMATE}")));
    doc
}

/// Backslash-escape the characters that Markdown or GFM tables treat as syntax.
fn md_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|' | '~' | '&' | '!' | '#'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn to_markdown(doc: &Doc) -> String {
    let mut out = String::new();
    writeln!(out, "# {}\n", doc.title).expect("write to String");
    for block in &doc.blocks {
        match block {
            Block::Heading(text) => writeln!(out, "## {text}\n"),
            Block::Para(text) => writeln!(out, "{}\n", md_escape(text)),
            Block::Table { head, rows } => {
                writeln!(out, "| {} |", head.join(" | ")).expect("write to String");
                let rule = vec!["---"; head.len()].join(" | ");
                writeln!(out, "| {rule} |").expect("write to String");
                for row in rows {
                    let cells: Vec<String> = row.iter().map(|c| md_escape(c)).collect();
                    writeln!(out, "| {} |", cells.join(" | ")).expect("write to String");
                }
                writeln!(out)
            }
        }
        .expect("write to String");
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

const STYLE: &str = "body{font-family:sans-serif;max-width:60rem;margin:2rem auto;padding:0 1rem}\
table{border-collapse:collapse;margin-bottom:1rem}\
th,td{border:1px solid #888;padding:.25rem .6rem;text-align:left}\
th{background:#eee}";

fn to_html(doc: &Doc) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    writeln!(out, "<title>{}</title>", html_escape(doc.title)).expect("write to String");
    writeln!(out, "<style>{STYLE}</style>").expect("write to String");
    out.push_str("</head>\n<body>\n");
    writeln!(out, "<h1>{}</h1>", html_escape(doc.title)).expect("write to String");
    for block in &doc.blocks {
        match block {
            Block::Heading(text) => writeln!(out, "<h2>{}</h2>", html_escape(text)),
            Block::Para(text) => writeln!(out, "<p>{}</p>", html_escape(text)),
            Block::Table { head, rows } => {
                out.push_str("<table>\n<thead><tr>");
                for h in *head {
                    write!(out, "<th>{}</th>", html_escape(h)).expect("write to String");
                }
                out.push_str("</tr></thead>\n<tbody>\n");
                for row in rows {
                    out.push_str("<tr>");
                    for cell in row {
                        write!(out, "<td>{}</td>", html_escape(cell)).expect("write to String");
                    }
                    out.push_str("</tr>\n");
                }
                writeln!(out, "</tbody>\n</table>")
            }
        }
        .expect("write to String");
    }
    out.push_str("</body>\n</html>\n");
    out
}

/// Render the `count` report as Markdown. The string ends with a newline.
pub fn render_count_markdown(
    files: &[CountedFile<'_>],
    explain: bool,
    top: Option<u64>,
    costs: &[TierCost],
) -> String {
    to_markdown(&count_doc(files, explain, top, costs))
}

/// Render the `count` report as a self-contained HTML page.
pub fn render_count_html(
    files: &[CountedFile<'_>],
    explain: bool,
    top: Option<u64>,
    costs: &[TierCost],
) -> String {
    to_html(&count_doc(files, explain, top, costs))
}

/// Render the `diff` report as Markdown. The string ends with a newline.
pub fn render_diff_markdown(
    old: &Path,
    new: &Path,
    diff: &StateDiff,
    costs: &[CostDelta],
) -> String {
    to_markdown(&diff_doc(old, new, diff, costs))
}

/// Render the `diff` report as a self-contained HTML page.
pub fn render_diff_html(old: &Path, new: &Path, diff: &StateDiff, costs: &[CostDelta]) -> String {
    to_html(&diff_doc(old, new, diff, costs))
}
