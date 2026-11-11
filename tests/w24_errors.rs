//! W-24: every error path of the binary says what went wrong and what to do next.
//! One test per path. Exit codes: 2 input error, 3 usage error.

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .output()
        .expect("rum-meter binary")
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .into_os_string()
        .into_string()
        .expect("utf-8 path")
}

/// A sparse file one byte over the 128 MiB input cap. Nothing is written to disk.
fn oversize(name: &str) -> String {
    let path = scratch(name, b"");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open");
    file.set_len(128 * 1024 * 1024 + 1).expect("set_len");
    path
}

fn scratch(name: &str, bytes: &[u8]) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("w24");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    std::fs::write(&path, bytes).expect("write");
    path.into_os_string().into_string().expect("utf-8 path")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is utf-8")
}

/// Exit code, empty stdout, no panic, and a message that has the `needle` (what went
/// wrong) followed by the `action` (what to do next).
fn assert_refused(output: &Output, code: i32, needle: &str, action: &str) {
    let err = stderr(output);
    assert_eq!(output.status.code(), Some(code), "stderr: {err}");
    assert!(
        output.stdout.is_empty(),
        "stdout must stay empty on an error"
    );
    assert!(!err.contains("panicked"), "{err}");
    let at = err
        .find(needle)
        .unwrap_or_else(|| panic!("no {needle:?} in {err}"));
    assert!(
        err[at..].contains(action),
        "no {action:?} after {needle:?} in {err}"
    );
}

fn count_prices(case: &str, body: &str) -> Output {
    let prices = scratch(&format!("{case}.json"), body.as_bytes());
    run(&[
        "count",
        &fixture("v4_one_managed.json"),
        "--prices",
        &prices,
    ])
}

macro_rules! price_case {
    ($name:ident, $body:expr, $needle:expr, $action:expr) => {
        #[test]
        fn $name() {
            assert_refused(&count_prices(stringify!($name), $body), 2, $needle, $action);
        }
    };
}

// ---- state files ----

#[test]
fn state_missing_says_what_to_do() {
    let out = run(&["count", "no-such-state-file.json"]);
    assert_refused(&out, 2, "unreadable", "check that the path exists");
}

#[test]
fn state_directory_says_what_to_do() {
    let dir = env!("CARGO_TARGET_TMPDIR");
    assert_refused(
        &run(&["count", dir]),
        2,
        "not a regular file",
        "pass a regular file",
    );
}

#[test]
fn state_invalid_json_says_what_to_do() {
    let path = scratch("broken.json", b"{ nope");
    assert_refused(
        &run(&["count", &path]),
        2,
        "invalid json at line 1",
        "terraform state pull",
    );
}

#[test]
fn state_version_3_says_what_to_do() {
    let out = run(&["count", &fixture("version_3.json")]);
    assert_refused(&out, 2, "state version is not 4", "terraform state pull");
}

#[test]
fn state_array_says_what_to_do() {
    let path = scratch("array.json", b"[]");
    assert_refused(
        &run(&["count", &path]),
        2,
        "state is not an object",
        "one JSON object",
    );
}

#[test]
fn state_utf16_says_what_to_do() {
    let path = scratch("utf16.json", &[0xff, 0xfe, b'{', 0, b'}', 0]);
    assert_refused(&run(&["count", &path]), 2, "UTF-16", "save it as UTF-8");
}

#[test]
fn state_over_the_cap_says_what_to_do() {
    let path = oversize("big.json");
    assert_refused(
        &run(&["count", &path]),
        2,
        "larger than",
        "pass a smaller file",
    );
}

#[test]
fn diff_old_state_missing_says_what_to_do() {
    let out = run(&["diff", "no-such-old.json", &fixture("v4_diff_new.json")]);
    assert_refused(&out, 2, "unreadable", "check that the path exists");
}

// ---- price files ----

#[test]
fn prices_missing_says_what_to_do() {
    let out = run(&[
        "count",
        &fixture("v4_one_managed.json"),
        "--prices",
        "no-such-prices.json",
    ]);
    assert_refused(&out, 2, "unreadable", "check that the path exists");
}

#[test]
fn prices_directory_says_what_to_do() {
    let dir = env!("CARGO_TARGET_TMPDIR");
    let out = run(&["count", &fixture("v4_one_managed.json"), "--prices", dir]);
    assert_refused(&out, 2, "not a regular file", "pass a regular file");
}

#[test]
fn prices_over_the_cap_says_what_to_do() {
    let path = oversize("big-prices.json");
    let out = run(&["count", &fixture("v4_one_managed.json"), "--prices", &path]);
    assert_refused(&out, 2, "larger than", "pass a smaller file");
}

#[test]
fn prices_utf16_says_what_to_do() {
    let path = scratch("utf16-prices.json", &[0xff, 0xfe, b'{', 0, b'}', 0]);
    let out = run(&["count", &fixture("v4_one_managed.json"), "--prices", &path]);
    assert_refused(&out, 2, "UTF-16", "save it as UTF-8");
}

price_case!(
    prices_invalid_json,
    "{ nope",
    "invalid json at line 1",
    "terraform state pull"
);
price_case!(
    prices_not_an_object,
    "[]",
    "invalid price file",
    "see docs/reference.md"
);
price_case!(
    prices_without_tiers,
    r#"{"currency":"EUR","tiers":[]}"#,
    "has no tiers",
    "add at least one tier"
);
price_case!(
    prices_empty_tier_name,
    r#"{"currency":"EUR","tiers":[{"name":"","price_per_rum_hour":1}]}"#,
    "tier name is empty",
    "give each tier a name"
);
price_case!(
    prices_tier_not_an_object,
    r#"{"currency":"EUR","tiers":[1]}"#,
    "tier is not an object",
    "write each tier as a JSON object"
);
price_case!(
    prices_duplicate_key,
    r#"{"currency":"EUR","tiers":[{"name":"a","name":"b","price_per_rum_hour":1}]}"#,
    "duplicate key in a tier",
    "remove the repeated key"
);
price_case!(
    prices_duplicate_tier_name,
    r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":1},{"name":"a","price_per_rum_hour":2}]}"#,
    "duplicate tier name",
    "give each tier its own name"
);
price_case!(
    prices_negative,
    r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":-1}]}"#,
    "price is negative",
    "0 or more"
);
price_case!(
    prices_not_a_number,
    r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":true}]}"#,
    "price is not a number",
    "decimal number or string"
);
price_case!(
    prices_empty_currency,
    r#"{"currency":"","tiers":[{"name":"a","price_per_rum_hour":1}]}"#,
    "currency is empty",
    "such as EUR"
);
price_case!(
    prices_currency_too_long,
    r#"{"currency":"EURO-LONG-CODE-THAT-IS-TOO-LONG","tiers":[{"name":"a","price_per_rum_hour":1}]}"#,
    "currency is too long",
    "short code such as EUR"
);
price_case!(
    prices_currency_not_ascii,
    r#"{"currency":"€","tiers":[{"name":"a","price_per_rum_hour":1}]}"#,
    "currency is not visible ASCII",
    "letters only"
);
price_case!(
    prices_cost_overflow,
    r#"{"currency":"EUR","tiers":[{"name":"a","price_per_rum_hour":1e309}]}"#,
    "monthly cost overflows",
    "check the prices and --hours-per-month"
);

// ---- inputs that conflict ----

#[test]
fn count_with_a_repeated_state_says_what_to_do() {
    let state = fixture("v4_one_managed.json");
    assert_refused(
        &run(&["count", &state, &state]),
        2,
        "more than once",
        "pass each state file once",
    );
}

#[test]
fn count_with_a_repeated_address_says_what_to_do() {
    let one = fixture("v4_one_managed.json");
    let copy = scratch("copy.json", &std::fs::read(&one).expect("read"));
    let out = run(&["count", &one, &copy]);
    assert_refused(&out, 2, "duplicate address", "remove the repeat");
}

// ---- output and usage ----

#[test]
fn closed_stdout_says_what_to_do() {
    let Ok(full) = std::fs::OpenOptions::new().write(true).open("/dev/full") else {
        return;
    };
    let out = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(["count", &fixture("v4_one_managed.json")])
        .stdout(full)
        .stderr(Stdio::piped())
        .output()
        .expect("run");
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(
        err.contains("cannot write report to stdout; check that stdout is open"),
        "{err}"
    );
}

#[test]
fn unknown_option_exits_3_with_a_hint() {
    let out = run(&["count", "--no-such-option"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(err.contains("--help"), "{err}");
}

#[test]
fn count_without_a_state_exits_3_with_usage() {
    let out = run(&["count"]);
    let err = stderr(&out);
    assert_eq!(out.status.code(), Some(3), "{err}");
    assert!(err.contains("Usage"), "{err}");
}

#[test]
fn refused_flags_exit_3_and_name_the_valid_use() {
    let s = fixture("v4_one_managed.json");
    let d = fixture("v4_diff_new.json");
    for (args, text) in [
        (
            vec!["count", &s, "--max-increase", "1"],
            "--max-increase only works with diff",
        ),
        (
            vec!["diff", &s, &d, "--max-rum", "1"],
            "--max-rum only works with count",
        ),
        (
            vec!["count", &s, "--hours-per-month", "10"],
            "only works together with --prices",
        ),
        (
            vec!["diff", &s, &d, "--top", "1"],
            "--top with diff only works together with --json",
        ),
        (
            vec!["diff", &s, &d, "--explain"],
            "--explain with diff only works together with --json",
        ),
    ] {
        let out = run(&args);
        assert_refused(&out, 3, text, text);
    }
}
