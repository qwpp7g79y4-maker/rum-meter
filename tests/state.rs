use std::path::PathBuf;
use std::process::{Command, Output};

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
        .expect("fixture path is utf-8")
}

fn assert_stdout_empty(output: &Output) {
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty on an input error"
    );
}

fn assert_invalid_json(output: &Output, path: &str) {
    assert_eq!(output.status.code(), Some(2), "invalid json should exit 2");
    assert_stdout_empty(output);
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    let prefix = format!("rum-meter: {path}: invalid json at line ");
    assert!(
        stderr.starts_with(&prefix),
        "error should name the path and the json kind"
    );
    let rest = stderr
        .strip_prefix(&prefix)
        .expect("prefix")
        .strip_suffix('\n')
        .expect("single trailing newline");
    let (line, column) = rest.split_once(" column ").expect("column");
    assert!(
        !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit()),
        "line should be a number"
    );
    assert!(
        !column.is_empty() && column.bytes().all(|byte| byte.is_ascii_digit()),
        "column should be a number"
    );
    assert_eq!(stderr.lines().count(), 1);
}

#[test]
fn version_3_exits_2() {
    let path = fixture("version_3.json");
    let output = run(&["count", &path]);
    assert_eq!(output.status.code(), Some(2), "version 3 should exit 2");
    assert_stdout_empty(&output);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: state version is not 4\n")
    );
}

#[test]
fn broken_json_exits_2() {
    let path = fixture("broken.json");
    let output = run(&["count", &path]);
    assert_invalid_json(&output, &path);
}

#[test]
fn version_string_exits_2_without_file_content() {
    let path = fixture("version_string.json");
    let output = run(&["count", &path]);
    assert_invalid_json(&output, &path);
}

#[test]
fn empty_file_exits_2() {
    let path = fixture("empty.json");
    let output = run(&["count", &path]);
    assert_invalid_json(&output, &path);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: invalid json at line 1 column 0\n")
    );
}

#[test]
fn missing_file_exits_2() {
    let path = fixture("missing.json");
    let output = run(&["count", &path]);
    assert_eq!(output.status.code(), Some(2), "missing file should exit 2");
    assert_stdout_empty(&output);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: unreadable (not found)\n")
    );
}

#[test]
fn directory_exits_2() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .into_os_string()
        .into_string()
        .expect("utf-8");
    let output = run(&["count", &path]);
    assert_eq!(output.status.code(), Some(2), "directory should exit 2");
    assert_stdout_empty(&output);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(stderr, format!("rum-meter: {path}: not a regular file\n"));
}

#[test]
fn count_reads_every_file() {
    let good = fixture("v4_handwritten.json");
    let bad = fixture("empty.json");
    let output = run(&["count", &good, &bad]);
    assert_invalid_json(&output, &bad);
}

#[test]
fn diff_rejects_version_3() {
    let old = fixture("version_3.json");
    let new = fixture("v4_handwritten.json");
    let output = run(&["diff", &old, &new]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "diff version 3 should exit 2"
    );
    assert_stdout_empty(&output);
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {old}: state version is not 4\n")
    );
}

#[test]
fn count_valid_state_exits_0_without_file_content() {
    let path = fixture("v4_handwritten.json");
    let output = run(&["count", &path]);
    assert_eq!(output.status.code(), Some(0), "valid state should exit 0");
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(
        stdout
            .contains("estimate: HCP Terraform bills the hourly peak; this is a single snapshot\n")
    );
    for canary in [
        "CANARY-SECRET-S-ATTR",
        "CANARY-SECRET-S-SENSITIVE",
        "CANARY-SECRET-S-PRIVATE",
        "CANARY-SECRET-S-OUTPUT",
    ] {
        assert!(!stdout.contains(canary), "stdout leaked {canary}");
    }
}
