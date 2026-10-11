//! Exit codes 0, 1, 2 and 3.
//!
//! `--max-rum` limits `count` (the sum across the given states).
//! `--max-increase` limits `diff` (new RUM minus old RUM).
//! A total or increase equal to the limit is allowed.
//! `--max-increase` on `count`, and `--max-rum` on `diff`, exit 3.
//! A breach still prints the same report and writes nothing to stderr.
//! Unreadable input stays exit 2 with an empty stdout, even when the loaded
//! part would already be over the limit.

use std::process::{Command, Output};

const HANDWRITTEN: &str = "tests/fixtures/v4_handwritten.json";
const ONE: &str = "tests/fixtures/v4_one_managed.json";
const DIFF_OLD: &str = "tests/fixtures/v4_diff_old.json";
const DIFF_NEW: &str = "tests/fixtures/v4_diff_new.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn assert_usage(output: &Output, message: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let leaked = output
        .stdout
        .windows(b"CANARY".len())
        .any(|window| window == b"CANARY")
        || stderr.contains("CANARY");
    assert!(
        output.status.code() == Some(3)
            && output.stdout.is_empty()
            && stderr == format!("{message}\n")
            && !leaked,
        "exit {:?} stdout bytes {} stderr bytes {} leaked {leaked}",
        output.status.code(),
        output.stdout.len(),
        stderr.len()
    );
}

fn assert_same_report(output: &Output, code: i32, baseline: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stderr.is_empty(), "stderr:\n{stderr}");
    assert_eq!(output.stdout, baseline.stdout);
    assert!(
        !stdout.contains("CANARY-SECRET"),
        "stdout leaked a fixture value:\n{stdout}"
    );
}

#[test]
fn count_equal_to_max_rum_exits_0() {
    let base = run(&["count", HANDWRITTEN]);
    assert_eq!(base.status.code(), Some(0));
    let at_limit = run(&["count", "--max-rum", "2", HANDWRITTEN]);
    assert_same_report(&at_limit, 0, &base);
    let under = run(&["count", "--max-rum", "3", HANDWRITTEN]);
    assert_same_report(&under, 0, &base);
}

#[test]
fn count_over_max_rum_exits_1() {
    let base = run(&["count", HANDWRITTEN]);
    let stdout = String::from_utf8(base.stdout.clone()).expect("utf-8");
    assert!(
        stdout.contains("total rum: 2\n"),
        "fixture total changed:\n{stdout}"
    );
    let over = run(&["count", "--max-rum", "1", HANDWRITTEN]);
    assert_same_report(&over, 1, &base);
}

#[test]
fn count_limit_uses_the_sum_of_every_file() {
    let base = run(&["count", HANDWRITTEN, ONE]);
    let stdout = String::from_utf8(base.stdout.clone()).expect("utf-8");
    assert!(
        stdout.contains("total rum: 3\n"),
        "fixture sum changed:\n{stdout}"
    );
    let at_limit = run(&["count", "--max-rum", "3", HANDWRITTEN, ONE]);
    assert_same_report(&at_limit, 0, &base);
    let over = run(&["count", "--max-rum", "2", HANDWRITTEN, ONE]);
    assert_same_report(&over, 1, &base);
}

#[test]
fn count_max_increase_exits_3() {
    let output = run(&["count", "--max-increase", "0", HANDWRITTEN]);
    assert_usage(&output, "--max-increase only works with diff");
}

#[test]
fn count_explain_and_json_keep_the_report_on_exit_1() {
    let text = run(&["count", "--explain", HANDWRITTEN]);
    let text_over = run(&["count", "--explain", "--max-rum", "1", HANDWRITTEN]);
    assert_same_report(&text_over, 1, &text);

    let json = run(&["count", "--json", HANDWRITTEN]);
    let json_over = run(&["count", "--json", "--max-rum", "1", HANDWRITTEN]);
    assert_same_report(&json_over, 1, &json);
    let stdout = String::from_utf8(json_over.stdout).expect("utf-8");
    assert!(stdout.contains("\"total_rum\": 2"));
}

#[test]
fn diff_increase_equal_to_max_increase_exits_0() {
    let base = run(&["diff", DIFF_NEW, DIFF_OLD]);
    let stdout = String::from_utf8(base.stdout.clone()).expect("utf-8");
    assert!(
        stdout.contains("rum delta: +2\n"),
        "fixture delta changed:\n{stdout}"
    );
    let at_limit = run(&["diff", "--max-increase", "2", DIFF_NEW, DIFF_OLD]);
    assert_same_report(&at_limit, 0, &base);
    let under = run(&["diff", "--max-increase", "3", DIFF_NEW, DIFF_OLD]);
    assert_same_report(&under, 0, &base);
}

#[test]
fn diff_increase_over_max_increase_exits_1() {
    let base = run(&["diff", DIFF_NEW, DIFF_OLD]);
    let over = run(&["diff", "--max-increase", "1", DIFF_NEW, DIFF_OLD]);
    assert_same_report(&over, 1, &base);
    let zero = run(&["diff", "--max-increase", "0", DIFF_NEW, DIFF_OLD]);
    assert_same_report(&zero, 1, &base);
}

#[test]
fn diff_decrease_does_not_exceed_max_increase() {
    let base = run(&["diff", DIFF_OLD, DIFF_NEW]);
    let stdout = String::from_utf8(base.stdout.clone()).expect("utf-8");
    assert!(
        stdout.contains("rum delta: -2\n"),
        "fixture delta changed:\n{stdout}"
    );
    let output = run(&["diff", "--max-increase", "0", DIFF_OLD, DIFF_NEW]);
    assert_same_report(&output, 0, &base);
}

#[test]
fn diff_max_rum_exits_3() {
    let output = run(&["diff", "--max-rum", "0", DIFF_OLD, DIFF_NEW]);
    assert_usage(&output, "--max-rum only works with count");
}

#[test]
fn diff_json_keeps_the_report_on_exit_1() {
    let base = run(&["diff", "--json", DIFF_NEW, DIFF_OLD]);
    let over = run(&["diff", "--json", "--max-increase", "1", DIFF_NEW, DIFF_OLD]);
    assert_same_report(&over, 1, &base);
    let stdout = String::from_utf8(over.stdout).expect("utf-8");
    assert!(stdout.contains("\"rum_delta\": 2"));
}

#[test]
fn input_error_wins_over_a_threshold_breach() {
    let bad = "tests/fixtures/missing.json";
    let output = run(&["count", "--max-rum", "0", HANDWRITTEN, bad]);
    let stderr = String::from_utf8(output.stderr).expect("utf-8");
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout leaked a report:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(output.stdout.is_empty(), "stdout should stay empty");
    assert_eq!(
        stderr,
        format!("rum-meter: {bad}: unreadable (not found)\n")
    );
    assert!(!stderr.contains("CANARY-SECRET"));
}

#[test]
fn non_numeric_threshold_exits_3() {
    let count = run(&["count", "--max-rum", "nope", HANDWRITTEN]);
    assert_eq!(count.status.code(), Some(3));
    assert!(count.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&count.stderr).contains("CANARY-SECRET"));

    let diff = run(&["diff", "--max-increase", "-1", DIFF_OLD, DIFF_NEW]);
    assert_eq!(diff.status.code(), Some(3));
    assert!(diff.stdout.is_empty());
}
