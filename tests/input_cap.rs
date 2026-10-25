//! A state file, a price file, or stdin larger than 128 MiB is refused.
//!
//! Each file here is sparse (`set_len`, no disk blocks), so the test does not
//! allocate or write the bytes it claims to have; it only proves the reader
//! stops at the cap instead of reading the full logical size.

use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const OVER_CAP: u64 = 129 * 1024 * 1024;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .stdin(Stdio::null())
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

fn sparse_file(name: &str) -> String {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("input-cap");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let path = dir.join(name);
    let file = File::create(&path).expect("create sparse file");
    file.set_len(OVER_CAP).expect("set_len");
    drop(file);
    path.into_os_string().into_string().expect("utf-8 path")
}

fn assert_too_large(output: &Output, path: &str, elapsed: Duration) {
    assert_eq!(
        output.status.code(),
        Some(2),
        "a file over the cap should exit 2"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout should be empty, len {}",
        output.stdout.len()
    );
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: file is larger than 128 MiB\n")
    );
    assert!(
        elapsed < Duration::from_secs(5),
        "refusal took {elapsed:?}, the sparse file should not be read in full"
    );
}

#[test]
fn sparse_state_file_over_the_cap_is_refused() {
    let path = sparse_file("sparse-state.json");
    let start = Instant::now();
    let output = run(&["count", &path]);
    assert_too_large(&output, &path, start.elapsed());
}

#[test]
fn sparse_price_file_over_the_cap_is_refused() {
    let state = fixture("v4_one_managed.json");
    let path = sparse_file("sparse-prices.json");
    let start = Instant::now();
    let output = run(&["count", &state, "--prices", &path]);
    assert_too_large(&output, &path, start.elapsed());
}

/// Stdin reads the same capped loop as a named file. A sparse file redirected
/// onto stdin exercises it without a real pipe or 129 MiB of real bytes.
#[test]
fn sparse_stdin_over_the_cap_is_refused() {
    let path = sparse_file("sparse-stdin.json");
    let stdin_file = File::open(&path).expect("open sparse file for stdin");
    let start = Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .arg("count")
        .arg("-")
        .stdin(Stdio::from(stdin_file))
        .output()
        .expect("rum-meter binary");
    assert_too_large(&output, "-", start.elapsed());
}
