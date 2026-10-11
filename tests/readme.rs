//! Documentation tests: README.md, README.nl.md, docs/reference.md and
//! docs/reference.nl.md.
//!
//! Every ```console example is run against the binary. Every exit-2 and exit-3
//! row in the usage-error tables is run against the binary. The English and
//! Dutch pages must have the same sections, the same source URLs and the same
//! examples.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const README: &str = "README.md";
const README_NL: &str = "README.nl.md";
const REF: &str = "docs/reference.md";
const REF_NL: &str = "docs/reference.nl.md";

const HASHICORP_RUM_HOUR_PRICES: [&str; 6] = [
    "0.0001359",
    "0.0001418",
    "0.0001132",
    "0.0001182",
    "0.0001057",
    "0.0001103",
];

const QUOTES: [&str; 4] = [
    "Each infrastructure object, including but not limited to “Local Only” Resources, counts as a single Resource.",
    "A managed resource is a resource in an HCP Terraform-managed state file where `mode = \"managed\"`. HCP Terraform counts a resource as part of this count starting from the first `terraform plan` or `terraform apply` operation on the resource.",
    "HCP Terraform includes resources in modules and each resource instance created with the `count` or `for_each` meta-arguments.",
    "Per Hour Price x 24 x 30",
];

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd
}

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

/// The text under a level-2 heading, up to the next level-2 heading.
fn section<'a>(text: &'a str, heading: &str) -> &'a str {
    let marker = format!("{heading}\n");
    let start = text
        .find(&marker)
        .unwrap_or_else(|| panic!("missing heading {heading}"))
        + marker.len();
    let rest = &text[start..];
    match rest.find("\n## ") {
        Some(end) => &rest[..end],
        None => rest,
    }
}

fn headings(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.starts_with("## "))
        .collect()
}

fn urls(text: &str) -> BTreeSet<String> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|word| word.starts_with("https://"))
        .map(|word| word.trim_end_matches(['.', ',', '"']).to_string())
        .collect()
}

fn backtick(cell: &str) -> &str {
    let cell = cell.trim();
    cell.strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .unwrap_or_else(|| panic!("cell not in backticks: {cell}"))
}

/// `\n` in a table cell is a real newline. The cell itself has none.
fn message_of(cell: &str) -> String {
    let mut text = backtick(cell).replace("\\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

struct Row {
    call: String,
    args: Vec<String>,
    exit: i32,
    message: String,
}

fn error_rows(doc: &str, heading: &str) -> Vec<Row> {
    let part = section(doc, heading);
    let mut rows = Vec::new();
    for line in part.lines().filter(|line| line.starts_with('|')) {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() < 8 {
            continue;
        }
        let cells: Vec<&str> = parts[1..7].iter().map(|cell| cell.trim()).collect();
        let exit: i32 = match cells[4] {
            "2" => 2,
            "3" => 3,
            _ => continue,
        };
        let call = backtick(cells[3]).to_string();
        let args = call.split_whitespace().map(str::to_string).collect();
        rows.push(Row {
            call,
            args,
            exit,
            message: message_of(cells[5]),
        });
    }
    assert!(!rows.is_empty(), "{heading} has no error row");
    rows
}

/// Every ```console block: the first line is `$ rum-meter ARGS`, the rest is stdout.
fn examples(doc: &str) -> Vec<(Vec<String>, String)> {
    let mut out = Vec::new();
    let mut lines = doc.lines();
    while let Some(line) = lines.next() {
        if line != "```console" {
            continue;
        }
        let call = lines.next().expect("empty console block");
        let args: Vec<String> = call
            .strip_prefix("$ rum-meter ")
            .unwrap_or_else(|| panic!("console block does not start with `$ rum-meter `: {call}"))
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let mut body = String::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "```" {
                closed = true;
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        assert!(closed, "console block not closed: {call}");
        out.push((args, body));
    }
    out
}

fn assert_no_planted_values(label: &str, text: &str) {
    assert!(
        !text.contains("CANARY-SECRET"),
        "{label} contains a planted fixture value"
    );
    for price in HASHICORP_RUM_HOUR_PRICES {
        assert!(
            !text.contains(price),
            "{label} contains a HashiCorp RUM-hour price"
        );
    }
}

#[test]
fn readme_has_the_sections_and_names() {
    let readme = read(README);
    assert_no_planted_values(README, &readme);
    for heading in [
        "## Why",
        "## Installation",
        "## System requirements",
        "## Quick start",
        "## How it counts",
        "## Compared with alternatives",
        "## Limitations",
        "## Privacy",
        "## Support",
        "## License",
        "## Contact",
    ] {
        assert!(
            readme.lines().any(|line| line == heading),
            "missing {heading}"
        );
    }
    let limits = section(&readme, "## Limitations");
    for part in [
        "500 managed resources",
        "a limit, not a price",
        "so not a pipe or process substitution",
        "Only state format v4",
    ] {
        assert!(limits.contains(part), "Limitations is missing {part}");
    }
    assert!(section(&readme, "## Why").contains("Willingness to pay was not investigated"));
    assert!(section(&readme, "## Why").contains("7 October 2026"));
    assert!(section(&readme, "## License").contains("PEPAI"));
    assert!(section(&readme, "## License").contains("at your option"));
    assert!(section(&readme, "## Contact").contains("pep_ai@icloud.com"));
    assert!(readme.contains("not a HashiCorp rate"));
    assert!(readme.contains("Estimate HCP Terraform cost"));
    assert!(readme.contains("Hourly Peak Managed Resources"));
    for message in [
        "--max-increase only works with diff",
        "--max-rum only works with count",
        "--hours-per-month only works together with --prices",
        "--top with diff only works together with --json",
        "--explain with diff only works together with --json",
    ] {
        assert!(readme.contains(message), "README is missing {message}");
    }
}

#[test]
fn language_switch_on_top_of_every_page() {
    for (rel, line) in [
        (README, "English · [Nederlands](README.nl.md)"),
        (README_NL, "[English](README.md) · Nederlands"),
        (REF, "English · [Nederlands](reference.nl.md)"),
        (REF_NL, "[English](reference.md) · Nederlands"),
    ] {
        let text = read(rel);
        let third = text.lines().nth(2).unwrap_or_default();
        assert_eq!(third, line, "{rel} has no language switch on line 3");
    }
}

#[test]
fn english_and_dutch_pages_match() {
    for (en, nl) in [(README, README_NL), (REF, REF_NL)] {
        let en_text = read(en);
        let nl_text = read(nl);
        assert_eq!(
            headings(&en_text).len(),
            headings(&nl_text).len(),
            "{en} and {nl} have a different number of sections"
        );
        assert_eq!(
            urls(&en_text),
            urls(&nl_text),
            "{en} and {nl} cite different URLs"
        );
        assert_eq!(
            examples(&en_text),
            examples(&nl_text),
            "{en} and {nl} have different examples"
        );
    }
    let nl = read(README_NL);
    assert_eq!(headings(&nl).len(), 11, "README.nl.md section count");
    assert_eq!(headings(&read(REF)).len(), 7, "reference section count");
    assert!(nl.contains("7 oktober 2026"), "Dutch README date");
    assert!(
        read(REF_NL).contains("7 oktober 2026"),
        "Dutch reference date"
    );
}

#[test]
fn licenses_and_manifest_name_pepai() {
    assert!(read("LICENSE-MIT").contains("Copyright (c) 2026 PEPAI"));
    assert!(read("LICENSE-APACHE").contains("Copyright 2026 PEPAI"));
    let cargo = read("Cargo.toml");
    assert!(cargo.contains("PEPAI <pep_ai@icloud.com>"), "{cargo}");
    assert!(
        cargo.contains("Estimate billable HCP Terraform RUM from local state files"),
        "{cargo}"
    );
}

#[test]
fn sources_are_quoted_verbatim() {
    let readme = read(README);
    let compared = section(&readme, "## Compared with alternatives");
    let compared_nl = read(README_NL);
    let reference = read(REF);
    let reference_nl = read(REF_NL);
    assert!(compared.contains("7 October 2026"));
    assert!(compared.contains("not verified"));
    assert!(reference.contains("7 October 2026"));
    assert!(reference.contains("not verified"));
    for part in QUOTES {
        assert!(compared.contains(part), "comparison is missing {part}");
        assert!(
            compared_nl.contains(part),
            "Dutch comparison is missing {part}"
        );
        assert!(reference.contains(part), "reference is missing {part}");
        assert!(
            reference_nl.contains(part),
            "Dutch reference is missing {part}"
        );
    }
    let readme_limits = read(README);
    let limits = section(&readme_limits, "## Limitations");
    let limits_nl = read(README_NL);
    for part in [
        "Each partial hour is billed as a full hour",
        "HCP Terraform combines workspace and Stacks resources under management when calculating costs",
        "500 managed resources",
    ] {
        assert!(limits.contains(part), "Limitations is missing {part}");
        assert!(
            limits_nl.contains(part),
            "Dutch limitations are missing {part}"
        );
        assert!(reference.contains(part), "reference is missing {part}");
        assert!(
            reference_nl.contains(part),
            "Dutch reference is missing {part}"
        );
    }
    assert!(
        read(README).contains("Assumption: an unknown mode (not `managed` and not `data`) is not a managed resource, does not count, keeps R2, and gets the reason `unknown mode`.")
    );
    assert!(read(README).contains("Assumption: `terraform_data` falls under \"Null Resources\"."));
    for page in [
        README,
        README_NL,
        REF,
        REF_NL,
        "CHANGELOG.md",
        "CONTRIBUTING.md",
    ] {
        assert_no_planted_values(page, &read(page));
    }
}

#[test]
fn examples_match_the_binary() {
    let prices = [
        "count",
        "--prices",
        "tests/fixtures/example-prices.json",
        "tests/fixtures/v4_one_managed.json",
    ];
    let hours = [
        "count",
        "--prices",
        "tests/fixtures/example-prices.json",
        "--hours-per-month",
        "720",
        "tests/fixtures/v4_one_managed.json",
    ];
    for rel in [README, README_NL] {
        let doc = read(rel);
        let mut saw_prices = false;
        let mut saw_hours = false;
        let mut count = 0;
        for (args, expected) in examples(&doc) {
            count += 1;
            let output = bin().args(&args).output().expect("start binary");
            let out = String::from_utf8(output.stdout).expect("stdout is utf-8");
            let err = String::from_utf8(output.stderr).expect("stderr is utf-8");
            assert_eq!(
                output.status.code(),
                Some(0),
                "{rel} {args:?}\nstderr: {err}"
            );
            assert!(err.is_empty(), "{rel} {args:?} stderr: {err}");
            assert_eq!(out, expected, "{rel} {args:?}");
            assert_no_planted_values("example stdout", &out);
            if args == prices {
                saw_prices = true;
                assert!(expected.contains("example-a: EUR 730.00\n"));
                assert!(expected.contains("example-b: EUR 365.00\n"));
            }
            if args == hours {
                saw_hours = true;
                assert!(expected.contains("example-a: EUR 720.00\n"));
                assert!(expected.contains("example-b: EUR 360.00\n"));
            }
        }
        assert_eq!(count, 7, "{rel} has {count} examples");
        assert!(saw_prices, "{rel} is missing the 730 hour example");
        assert!(saw_hours, "{rel} is missing the 720 hour example");
    }
}

#[test]
fn price_example_is_the_fixture() {
    let fixture = read("tests/fixtures/example-prices.json");
    let block = format!("```json\n{fixture}");
    for rel in [README, README_NL, REF, REF_NL] {
        assert!(
            read(rel).contains(block.trim_end()),
            "{rel}: price example differs from tests/fixtures/example-prices.json"
        );
    }
}

#[test]
fn option_and_error_tables_match_the_binary() {
    let help = bin().arg("--help").output().expect("start binary");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("help is utf-8");
    let mut required = BTreeSet::new();
    for call in [
        "count --max-increase 0 tests/fixtures/v4_one_managed.json",
        "count --explain --hours-per-month 1 --max-increase 0 tests/fixtures/v4_one_managed.json",
        "diff --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --explain --top 1 --hours-per-month 1 --max-rum 0 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "count --hours-per-month 1 tests/fixtures/v4_one_managed.json",
        "diff --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --explain --top 1 --json --hours-per-month 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --explain --top 1 tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "diff --explain tests/fixtures/v4_diff_old.json tests/fixtures/v4_diff_new.json",
        "count",
        "diff only.json",
        "--not-an-option",
        "count --top abc tests/fixtures/v4_one_managed.json",
        "count tests/fixtures/missing.json",
        "count tests/fixtures",
        "count tests/fixtures/empty.json",
        "count tests/fixtures/broken.json",
        "count tests/fixtures/version_3.json",
        "count tests/fixtures/version_string.json",
        "count --prices tests/fixtures/v4_one_managed.json tests/fixtures/v4_one_managed.json",
        "count --prices tests/fixtures tests/fixtures/v4_one_managed.json",
        "count --prices tests/fixtures/missing-prices.json tests/fixtures/v4_one_managed.json",
        "count --prices tests/fixtures/empty.json tests/fixtures/v4_one_managed.json",
        "diff tests/fixtures/version_3.json tests/fixtures/v4_handwritten.json",
        "count /dev/null",
        "count --prices /dev/null tests/fixtures/v4_one_managed.json",
    ] {
        required.insert(call);
    }
    for (rel, options, errors) in [
        (REF, "## Options", "## Exit codes and usage errors"),
        (REF_NL, "## Opties", "## Exitcodes en gebruiksfouten"),
    ] {
        let doc = read(rel);
        let table = section(&doc, options);
        for line in help.lines() {
            let Some(start) = line.find("--") else {
                continue;
            };
            let name = line[start..].split_whitespace().next().unwrap();
            assert!(
                table.contains(name),
                "{rel}: option table is missing {name}"
            );
        }
        let mut calls = BTreeSet::new();
        for row in error_rows(&doc, errors) {
            // `/dev/null` is a device only on Unix; Windows has no such path.
            if cfg!(windows) && row.call.contains("/dev/") {
                calls.insert(row.call);
                continue;
            }
            let output = bin().args(&row.args).output().expect("start binary");
            let out = String::from_utf8(output.stdout).expect("stdout is utf-8");
            let err = String::from_utf8(output.stderr).expect("stderr is utf-8");
            assert_eq!(
                output.status.code(),
                Some(row.exit),
                "{rel} {:?} stderr: {err}",
                row.args
            );
            assert!(out.is_empty(), "{rel} {:?} stdout: {out}", row.args);
            assert_eq!(err, row.message, "{rel} {:?}", row.args);
            calls.insert(row.call);
        }
        for call in &required {
            assert!(calls.contains(*call), "{rel}: table is missing {call}");
        }
    }
}

#[test]
fn reference_describes_rules_and_price_errors() {
    let reference = read(REF);
    for part in [
        "(`0.5`, not `0,5`)",
        "`price is not a number`",
        "`price is negative`",
        "`price file has no tiers`",
        "`tier is not an object`",
        "`tier name is empty`",
        "`invalid price file`",
        "`currency is empty`",
        "`monthly cost overflows`",
        "`file is UTF-16; save it as UTF-8`",
        "`duplicate address`",
        "`not a regular file`",
        "`unreadable (not found)`",
        "`unreadable (permission denied)`",
        "`unreadable (is a directory)`",
        "`unreadable (io error)`",
        "`unknown mode`",
        "`data source`",
        "schema_version",
        "half away from zero",
        "730",
        "720",
    ] {
        assert!(reference.contains(part), "reference is missing {part}");
        assert!(
            read(REF_NL).contains(part),
            "Dutch reference is missing {part}"
        );
    }
}

#[test]
fn comma_price_utf16_and_duplicate_match_the_binary() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("readme-errors");
    fs::create_dir_all(&dir).expect("temp dir");
    let state =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v4_one_managed.json");

    let comma = dir.join("comma.json");
    fs::write(
        &comma,
        r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":"0,5"}]}"#,
    )
    .expect("write");
    let err = run_prices(&comma, &state);
    assert!(err.contains("price is not a number"), "{err}");

    let bare = dir.join("bare.json");
    fs::write(
        &bare,
        r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":0,5}]}"#,
    )
    .expect("write");
    let err = run_prices(&bare, &state);
    assert!(err.contains("invalid json at line"), "{err}");

    let negative = dir.join("negative.json");
    fs::write(
        &negative,
        r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":-1}]}"#,
    )
    .expect("write");
    assert!(run_prices(&negative, &state).contains("price is negative"));

    let no_tiers = dir.join("no-tiers.json");
    fs::write(&no_tiers, r#"{"currency":"EUR"}"#).expect("write");
    assert!(run_prices(&no_tiers, &state).contains("price file has no tiers"));

    let tier = dir.join("tier.json");
    fs::write(&tier, r#"{"currency":"EUR","tiers":["x"]}"#).expect("write");
    assert!(run_prices(&tier, &state).contains("tier is not an object"));

    let empty_name = dir.join("empty-name.json");
    fs::write(
        &empty_name,
        r#"{"currency":"EUR","tiers":[{"name":"","price_per_rum_hour":1}]}"#,
    )
    .expect("write");
    assert!(run_prices(&empty_name, &state).contains("tier name is empty"));

    let array = dir.join("array.json");
    fs::write(&array, "[1]").expect("write");
    assert!(run_prices(&array, &state).contains("invalid price file"));

    let huge = dir.join("huge.json");
    fs::write(
        &huge,
        r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":1e308}]}"#,
    )
    .expect("write");
    assert!(run_prices(&huge, &state).contains("monthly cost overflows"));

    let utf16 = dir.join("utf16.json");
    fs::write(&utf16, [0xFF, 0xFE, 0x7B, 0x00]).expect("write");
    let output = bin()
        .arg("count")
        .arg(&utf16)
        .output()
        .expect("start binary");
    assert_eq!(output.status.code(), Some(2));
    let err = String::from_utf8(output.stderr).expect("utf-8");
    assert!(err.contains("file is UTF-16; save it as UTF-8"), "{err}");
    assert!(output.stdout.is_empty());

    let dup = dir.join("dup.json");
    fs::write(
        &dup,
        r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"a","instances":[{"index_key":1},{"index_key":1}]}]}"#,
    )
    .expect("write");
    let output = bin().arg("count").arg(&dup).output().expect("start binary");
    assert_eq!(output.status.code(), Some(2));
    let err = String::from_utf8(output.stderr).expect("utf-8");
    assert!(err.contains("duplicate address"), "{err}");
    assert!(output.stdout.is_empty());
}

fn run_prices(prices: &Path, state: &Path) -> String {
    let output = bin()
        .arg("count")
        .arg("--prices")
        .arg(prices)
        .arg(state)
        .output()
        .expect("start binary");
    assert_eq!(output.status.code(), Some(2), "{prices:?}");
    assert!(output.stdout.is_empty());
    String::from_utf8(output.stderr).expect("utf-8")
}

#[test]
fn fixture_readme_describes_every_file_as_synthetic() {
    let text = read("tests/fixtures/README.md");
    assert_no_planted_values("tests/fixtures/README.md", &text);
    assert!(text.contains("terraform is not installed"));
    assert!(text.contains("written by hand"));
    assert!(text.contains("not a HashiCorp rate"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut count = 0;
    walk(&root, "", &text, &mut count);
    assert!(count > 0);
}

fn walk(dir: &Path, rel: &str, text: &str, count: &mut usize) {
    for entry in fs::read_dir(dir).expect("fixtures") {
        let entry = entry.expect("entry");
        let name = entry.file_name().into_string().expect("utf-8");
        if name.starts_with('.')
            || name.ends_with(".generated")
            || name.ends_with(".backup")
            || name.ends_with(".lock.hcl")
            || name == "README.md"
        {
            continue;
        }
        let path = entry.path();
        let child = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };
        if path.is_dir() {
            if rel.is_empty() {
                let part = section(text, &format!("## {name}"));
                assert!(
                    part.to_lowercase().contains("synthetic"),
                    "{name} is not described as synthetic"
                );
            }
            walk(&path, &child, text, count);
        } else {
            *count += 1;
            if rel.is_empty() {
                let part = section(text, &format!("## {name}"));
                assert!(
                    part.to_lowercase().contains("synthetic"),
                    "{name} is not described as synthetic"
                );
            } else {
                assert!(
                    text.contains(&child),
                    "fixture README does not name {child}"
                );
            }
        }
    }
}
