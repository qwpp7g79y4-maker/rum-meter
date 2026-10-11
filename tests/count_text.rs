use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const ESTIMATE: &str = "estimate: HCP Terraform bills the hourly peak; this is a single snapshot";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary")
}

fn assert_snapshot(name: &str, args: &[&str]) {
    let output = run(args);
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(0),
        "count should exit 0\nstderr:\n{stderr}\nstdout:\n{stdout}"
    );
    assert!(stderr.is_empty(), "stderr should be empty:\n{stderr}");
    assert!(
        !stdout.contains("CANARY-SECRET"),
        "stdout leaked a fixture value:\n{stdout}"
    );
    assert_eq!(
        stdout.lines().filter(|line| *line == ESTIMATE).count(),
        1,
        "the estimate line should appear once:\n{stdout}"
    );
    assert!(
        stdout.ends_with(&format!("{ESTIMATE}\n")),
        "the estimate line should close the report:\n{stdout}"
    );
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(name);
    let expected = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("snapshot {}: {err}", path.display()));
    assert_eq!(stdout, expected, "snapshot {name}");
}

#[test]
fn count_one_file_matches_snapshot() {
    assert_snapshot(
        "count_one.txt",
        &["count", "tests/fixtures/v4_handwritten.json"],
    );
}

#[test]
fn count_one_file_explain_matches_snapshot() {
    assert_snapshot(
        "count_one_explain.txt",
        &["count", "--explain", "tests/fixtures/v4_handwritten.json"],
    );
}

#[test]
fn count_two_files_sums_and_matches_snapshot() {
    assert_snapshot(
        "count_two.txt",
        &[
            "count",
            "tests/fixtures/v4_handwritten.json",
            "tests/fixtures/v4_one_managed.json",
        ],
    );
}

#[test]
fn count_two_files_explain_matches_snapshot() {
    assert_snapshot(
        "count_two_explain.txt",
        &[
            "count",
            "--explain",
            "tests/fixtures/v4_handwritten.json",
            "tests/fixtures/v4_one_managed.json",
        ],
    );
}
