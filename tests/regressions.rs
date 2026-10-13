//! Regressions found during the functionality review. All inputs are synthetic.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn write_input(group: &str, name: &str, body: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("regressions")
        .join(group);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    path
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v4_one_managed.json")
}

fn run(command: &str, state: &Path, prices: &Path, json: bool, hours: &str) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.arg(command).arg(state);
    if command == "diff" {
        cmd.arg(state);
    }
    cmd.arg("--prices")
        .arg(prices)
        .arg("--hours-per-month")
        .arg(hours);
    if json {
        cmd.arg("--json");
    }
    cmd.output().unwrap()
}

#[test]
fn decimal_half_cent_rounds_up_in_count_and_diff() {
    let prices = write_input(
        "rounding",
        "prices.json",
        r#"{"currency":"EUR","tiers":[{"name":"tier","price_per_rum_hour":1.005}]}"#,
    );
    let empty = write_input("rounding", "empty.json", r#"{"version":4,"resources":[]}"#);
    for json in [false, true] {
        let count = run("count", &fixture(), &prices, json, "1");
        assert!(count.status.success());
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
        cmd.arg("diff")
            .arg(&empty)
            .arg(fixture())
            .arg("--prices")
            .arg(&prices)
            .args(["--hours-per-month", "1"]);
        if json {
            cmd.arg("--json");
        }
        let diff = cmd.output().unwrap();
        assert!(diff.status.success());
        if json {
            let count: serde_json::Value = serde_json::from_slice(&count.stdout).unwrap();
            let diff: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
            assert_eq!(count["costs"][0]["amount"], "1.01");
            assert_eq!(diff["costs"][0]["amount"], "1.01");
            assert_eq!(diff["cost_delta"][0]["amount"], "+1.01");
        } else {
            assert!(
                String::from_utf8(count.stdout)
                    .unwrap()
                    .contains("EUR 1.01\n")
            );
            assert!(
                String::from_utf8(diff.stdout)
                    .unwrap()
                    .contains("EUR +1.01\n")
            );
        }
    }
}

#[test]
fn zero_hours_with_a_large_price_is_zero_on_count_and_diff() {
    let state = write_input(
        "zero",
        "state.json",
        r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"x","instances":[{"index_key":0},{"index_key":1}]}]}"#,
    );
    let prices = write_input(
        "zero",
        "prices.json",
        r#"{"currency":"EUR","tiers":[{"name":"tier","price_per_rum_hour":1e308}]}"#,
    );
    for command in ["count", "diff"] {
        for json in [false, true] {
            let output = run(command, &state, &prices, json, "0");
            assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
            assert!(output.stderr.is_empty());
            assert!(String::from_utf8(output.stdout).unwrap().contains("0.00"));
        }
    }
}

#[test]
fn price_labels_escape_controls_in_text_and_preserve_them_in_json() {
    let prices = write_input(
        "labels",
        "prices.json",
        r#"{"currency":"EUR\u001b[31m","tiers":[{"name":"tier\nextra\t","price_per_rum_hour":1}]}"#,
    );
    for command in ["count", "diff"] {
        let text = run(command, &fixture(), &prices, false, "1");
        assert!(text.status.success());
        assert!(!text.stdout.contains(&0x1b));
        assert!(!text.stdout.contains(&b'\t'));
        assert!(
            String::from_utf8(text.stdout)
                .unwrap()
                .contains("tier\\u000aextra\\u0009: EUR\\u001b[31m")
        );
        let json = run(command, &fixture(), &prices, true, "1");
        assert!(json.status.success());
        let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(value["costs"][0]["name"], "tier\nextra\t");
        assert_eq!(value["costs"][0]["currency"], "EUR\u{1b}[31m");
        if command == "diff" {
            assert_eq!(value["cost_delta"][0]["name"], "tier\nextra\t");
        }
    }
}

#[test]
fn price_labels_escape_bidirectional_and_invisible_characters_in_text() {
    let prices = write_input(
        "format-chars",
        "prices.json",
        r#"{"currency":"EUR\u202e","tiers":[{"name":"a\u200bb\u2066c\ufeff","price_per_rum_hour":1}]}"#,
    );
    for command in ["count", "diff"] {
        let text = run(command, &fixture(), &prices, false, "1");
        assert!(text.status.success());
        let text = String::from_utf8(text.stdout).unwrap();
        for ch in ['\u{202e}', '\u{200b}', '\u{2066}', '\u{feff}'] {
            assert!(
                !text.contains(ch),
                "{command}: {ch:?} reached the text report"
            );
        }
        assert!(text.contains("a\\u200bb\\u2066c\\ufeff: EUR\\u202e"));
        let json = run(command, &fixture(), &prices, true, "1");
        let value: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert_eq!(value["costs"][0]["currency"], "EUR\u{202e}");
        assert_eq!(value["costs"][0]["name"], "a\u{200b}b\u{2066}c\u{feff}");
    }
}

#[cfg(unix)]
#[test]
fn file_paths_escape_controls_in_reports_and_errors() {
    let state = write_input(
        "paths",
        "state\n\u{1b}[31m.json",
        r#"{"version":4,"resources":[]}"#,
    );
    for args in [vec!["count"], vec!["diff"]] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
        cmd.args(&args).arg(&state);
        if args[0] == "diff" {
            cmd.arg(&state);
        }
        let output = cmd.output().unwrap();
        assert!(output.status.success());
        assert!(!output.stdout.contains(&0x1b));
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("state\\u000a\\u001b[31m.json"));
    }
    let prices = write_input("paths", "prices\n\u{1b}[31m.json", "{}");
    let output = run("count", &state, &prices, false, "1");
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.contains(&0x1b));
    assert_eq!(String::from_utf8(output.stderr).unwrap().lines().count(), 1);
    fs::write(&state, "{}").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .arg("count")
        .arg(&state)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.contains(&0x1b));
    assert_eq!(String::from_utf8(output.stderr).unwrap().lines().count(), 1);
}

#[cfg(target_os = "linux")]
#[test]
fn failed_stdout_returns_an_error_without_panicking() {
    for command in ["count", "diff"] {
        for json in [false, true] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
            cmd.arg(command).arg(fixture());
            if command == "diff" {
                cmd.arg(fixture());
            }
            if json {
                cmd.arg("--json");
            }
            let output = cmd
                .stdout(
                    fs::OpenOptions::new()
                        .write(true)
                        .open("/dev/full")
                        .unwrap(),
                )
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert_eq!(
                String::from_utf8(output.stderr).unwrap(),
                "rum-meter: cannot write report to stdout\n"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn closed_stdout_returns_an_error_without_panicking() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;

    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .arg("count")
        .arg(fixture())
        .stdout(OwnedFd::from(writer))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "rum-meter: cannot write report to stdout\n"
    );
}

#[test]
fn exact_decimal_price_is_zero_euros() {
    let prices = write_input(
        "exact-decimal",
        "prices.json",
        r#"{"currency":"EUR","tiers":[{"name":"tier","price_per_rum_hour":0.004999999999999999999}]}"#,
    );
    let output = run("count", &fixture(), &prices, false, "1");
    assert!(output.status.success(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("EUR 0.00\n"), "{text}");
    assert!(!text.contains("0.01"), "{text}");
}

#[test]
fn json_array_state_exits_2() {
    let state = write_input(
        "json-array",
        "array.json",
        r#"[4,[["managed","random_id","x",null,[[null,null,null]]]]]"#,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .arg("count")
        .arg(&state)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{:?}", output.stdout);
    assert!(output.stdout.is_empty());
    let err = String::from_utf8(output.stderr).unwrap();
    assert!(err.contains("state is not an object"), "{err}");
    assert!(!err.contains("managed"), "{err}");
}

#[test]
fn same_file_twice_is_not_counted_twice() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("regressions")
        .join("same-file");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("a.json"),
        r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"x","instances":[{}]}]}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .current_dir(&dir)
        .args(["count", "a.json", "a.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "rum-meter: a.json: duplicate address random_id.x\n"
    );
}
