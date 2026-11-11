//! Markdown and HTML reports: snapshots, escaping, flag conflicts, and that
//! text and JSON keep working.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn ok_stdout(args: &[&str]) -> String {
    let output = run(args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?} stderr: {stderr}");
    assert!(stderr.is_empty(), "{args:?} stderr: {stderr}");
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}

fn assert_snapshot(name: &str, args: &[&str]) {
    let stdout = ok_stdout(args);
    assert!(!stdout.contains("CANARY-SECRET"), "leaked a fixture value");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("snapshot {}: {err}", path.display()));
    assert_eq!(stdout, expected, "snapshot {name}");
}

const HAND: &str = "tests/fixtures/v4_handwritten.json";
const TOP: &str = "tests/fixtures/v4_top.json";
const OLD: &str = "tests/fixtures/v4_diff_old.json";
const NEW: &str = "tests/fixtures/v4_diff_new.json";
const PRICES: &str = "tests/fixtures/example-prices.json";

#[test]
fn count_markdown_explain_prices_matches_snapshot() {
    assert_snapshot(
        "count_explain_prices.md",
        &["count", "--markdown", "--explain", "--prices", PRICES, HAND],
    );
}

#[test]
fn count_markdown_two_files_matches_snapshot() {
    assert_snapshot("count_two.md", &["count", "--markdown", HAND, TOP]);
}

#[test]
fn count_html_explain_prices_matches_snapshot() {
    assert_snapshot(
        "count_explain_prices.html",
        &["count", "--html", "--explain", "--prices", PRICES, HAND],
    );
}

#[test]
fn diff_markdown_prices_matches_snapshot() {
    assert_snapshot(
        "diff_prices.md",
        &["diff", "--markdown", "--prices", PRICES, OLD, NEW],
    );
}

#[test]
fn diff_html_prices_matches_snapshot() {
    assert_snapshot(
        "diff_prices.html",
        &["diff", "--html", "--prices", PRICES, OLD, NEW],
    );
}

#[test]
fn html_is_self_contained() {
    let html = ok_stdout(&["count", "--html", "--explain", HAND]);
    assert!(html.starts_with("<!DOCTYPE html>\n"));
    assert!(html.ends_with("</html>\n"));
    for needle in ["<script", "http://", "https://", "src=", "href="] {
        assert!(!html.contains(needle), "unexpected {needle}");
    }
}

#[test]
fn markdown_and_html_carry_the_same_total_as_text_and_json() {
    let text = ok_stdout(&["count", HAND]);
    assert!(text.contains("total rum: 2\n"));
    assert!(ok_stdout(&["count", "--markdown", HAND]).contains("Total RUM: 2\n"));
    assert!(ok_stdout(&["count", "--html", HAND]).contains("<p>Total RUM: 2</p>"));
    assert!(ok_stdout(&["count", "--json", HAND]).contains("\"total_rum\": 2"));
}

#[test]
fn top_applies_to_markdown() {
    let full = ok_stdout(&["count", "--markdown", TOP]);
    let capped = ok_stdout(&["count", "--markdown", "--top", "1", TOP]);
    assert!(
        capped.len() < full.len(),
        "--top 1 should shorten the report"
    );
}

#[test]
fn format_flags_exclude_each_other() {
    for flags in [
        ["--json", "--markdown"],
        ["--json", "--html"],
        ["--markdown", "--html"],
    ] {
        let output = run(&["count", flags[0], flags[1], HAND]);
        assert_eq!(output.status.code(), Some(3), "{flags:?}");
        assert!(output.stdout.is_empty(), "{flags:?}");
    }
}

#[test]
fn diff_explain_still_needs_json_in_other_formats() {
    let output = run(&["diff", "--markdown", "--explain", OLD, NEW]);
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn threshold_exit_code_applies_to_markdown() {
    let output = run(&["count", "--markdown", "--max-rum", "1", HAND]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!output.stdout.is_empty());
}

#[test]
fn hostile_names_are_escaped() {
    let dir = std::env::temp_dir().join(format!("rum-meter-w04-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    let state = dir.join("hostile.json");
    fs::write(
        &state,
        r#"{"version":4,"terraform_version":"1.9.8","serial":1,"lineage":"x","outputs":{},
"resources":[{"mode":"managed","type":"random_id","name":"<script>a|b`c","provider":"p",
"instances":[{"schema_version":0,"attributes":{}}]}]}"#,
    )
    .expect("write state");
    let path = state.to_str().expect("utf-8 path");
    let html = ok_stdout(&["count", "--html", "--explain", path]);
    assert!(!html.contains("<script>"), "{html}");
    assert!(html.contains("&lt;script&gt;a|b`c"), "{html}");
    let md = ok_stdout(&["count", "--markdown", "--explain", path]);
    assert!(md.contains("\\<script\\>a\\|b\\`c"), "{md}");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn text_and_json_are_unchanged_by_default() {
    assert!(ok_stdout(&["count", HAND]).starts_with("file: "));
    assert!(ok_stdout(&["count", "--json", HAND]).starts_with("{\n"));
}
