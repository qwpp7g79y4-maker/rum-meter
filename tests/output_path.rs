//! `--output PATH` writes the report to a file. An existing file is replaced.
//! A missing parent directory is exit 2, the same code as a failed write to
//! stdout, and that directory is not created. Exit 1 stays the threshold.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn run(args: &[&str]) -> (Option<i32>, Vec<u8>, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("rum-meter binary");
    (output.status.code(), output.stdout, output.stderr)
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn scratch() -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "rum-meter-output-{}-{}-{}",
        std::process::id(),
        nanos,
        N.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).expect("create scratch dir");
    dir
}

#[test]
fn output_flag_writes_a_new_file() {
    let help = run(&["--help"]);
    assert_eq!(help.0, Some(0));
    let help_text = String::from_utf8(help.1).expect("help is utf-8");
    assert!(help_text.contains("--output"), "{help_text}");
    assert!(
        !help_text.contains("--out "),
        "no second flag next to --output"
    );

    let state = fixture("v4_one_managed.json");
    let (plain_code, expected, plain_err) = run(&["count", &state]);
    let plain_err = String::from_utf8_lossy(&plain_err);
    assert_eq!(plain_code, Some(0), "{plain_err}");
    assert!(!expected.is_empty());

    let root = scratch();
    let report = root.join("report.txt");
    assert!(!report.exists());
    let report_path = report.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run(&["count", "--output", report_path, &state]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(0), "{err}");
    assert!(stdout.is_empty(), "stdout must stay empty");
    assert!(stderr.is_empty(), "{err}");
    assert_eq!(fs::read(&report).expect("read report"), expected);

    let old = fixture("v4_diff_old.json");
    let new = fixture("v4_diff_new.json");
    let (diff_code, diff_expected, diff_err) = run(&["diff", &old, &new]);
    let diff_err = String::from_utf8_lossy(&diff_err);
    assert_eq!(diff_code, Some(0), "{diff_err}");
    let diff_report = root.join("diff.txt");
    let diff_path = diff_report.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run(&["diff", "--output", diff_path, &old, &new]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(0), "{err}");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert_eq!(
        fs::read(&diff_report).expect("read diff report"),
        diff_expected
    );

    let limited = root.join("limited.txt");
    let limited_path = limited.to_str().expect("utf-8 path");
    let (code, stdout, stderr) =
        run(&["count", "--output", limited_path, "--max-rum", "0", &state]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(1), "threshold stays exit 1: {err}");
    assert_ne!(code, Some(4), "no new exit code");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(stderr.is_empty(), "{err}");
    assert_eq!(fs::read(&limited).expect("read limited report"), expected);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn output_flag_overwrites_an_existing_file() {
    let root = scratch();
    let report = root.join("report.txt");
    let prior = b"PRIOR-BYTES-NOT-THE-REPORT\n";
    fs::write(&report, prior).unwrap();
    let state = fixture("v4_one_managed.json");
    let report_path = report.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run(&["count", "--output", report_path, &state]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(0), "{err}");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(stderr.is_empty(), "{err}");
    let (plain_code, expected, plain_err) = run(&["count", &state]);
    let plain_err = String::from_utf8_lossy(&plain_err);
    assert_eq!(plain_code, Some(0), "{plain_err}");
    let body = fs::read(&report).expect("read report");
    assert_eq!(body, expected);
    assert_ne!(body, prior);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn output_flag_missing_directory_exits_2() {
    let root = scratch();
    let missing_dir = root.join("no-such-dir");
    let report = missing_dir.join("report.txt");
    let state = fixture("v4_one_managed.json");
    let report_path = report.to_str().expect("utf-8 path");
    let (code, stdout, stderr) = run(&["count", "--output", report_path, &state]);
    let err = String::from_utf8_lossy(&stderr);
    assert_eq!(code, Some(2), "{err}");
    assert_ne!(code, Some(1), "a missing directory is not a threshold");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(err.contains("cannot write report to "), "{err}");
    assert!(err.contains("does not create it"), "{err}");
    assert!(!err.contains("panicked"), "{err}");
    assert!(!report.exists());
    assert!(
        !missing_dir.exists(),
        "a missing parent directory must not be created"
    );
    fs::remove_dir_all(&root).ok();
}
