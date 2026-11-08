//! A state file, a price file, or stdin larger than 128 MiB is refused.
//!
//! Each file here is sparse (`set_len`, no disk blocks), so the test does not
//! allocate or write the bytes it claims to have. These tests pin the exit
//! code and message. That the refusal reads no byte at all is measured in the
//! unit tests of `src/regular.rs` with a counting reader, not with a wall
//! clock here: a timing bound fails on a slow, busy runner.

use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

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

/// Deletes the over-cap file even when an assertion fails.
struct RemoveFile(PathBuf);

impl Drop for RemoveFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn assert_too_large(output: &Output, path: &str) {
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
        format!("rum-meter: {path}: file is larger than 128 MiB; pass a smaller file\n")
    );
}

#[test]
fn sparse_state_file_over_the_cap_is_refused() {
    let path = sparse_file("sparse-state.json");
    let output = run(&["count", &path]);
    assert_too_large(&output, &path);
}

#[test]
fn sparse_price_file_over_the_cap_is_refused() {
    let state = fixture("v4_one_managed.json");
    let path = sparse_file("sparse-prices.json");
    let output = run(&["count", &state, "--prices", &path]);
    assert_too_large(&output, &path);
}

/// Stdin reads the same capped loop as a named file. A sparse file redirected
/// onto stdin exercises it without a real pipe or 129 MiB of real bytes.
#[test]
fn sparse_stdin_over_the_cap_is_refused() {
    let path = sparse_file("sparse-stdin.json");
    let stdin_file = File::open(&path).expect("open sparse file for stdin");
    let output = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .arg("count")
        .arg("-")
        .stdin(Stdio::from(stdin_file))
        .output()
        .expect("rum-meter binary");
    assert_too_large(&output, "-");
}

/// 129 MiB, which is over the 128 MiB cap, outside the repository.
///
/// On Unix the file is created in `/tmp`. It is removed before the test
/// returns and is not committed. `set_len` leaves it sparse, so the test
/// writes no payload and the binary refuses the file from its size.
#[test]
fn file_outside_the_repo_over_the_cap_exits_2() {
    let dir = if cfg!(unix) {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let path = dir.join(format!(
        "rum-meter-w46-over-cap-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let _remove = RemoveFile(path.clone());
    let file = File::create(&path).expect("create over-cap file");
    file.set_len(OVER_CAP).expect("set_len");
    drop(file);
    assert!(
        !path.starts_with(env!("CARGO_MANIFEST_DIR")),
        "the over-cap file must stay out of the repository"
    );
    let path_str = path.to_str().expect("utf-8 path");
    let output = run(&["count", path_str]);
    assert_too_large(&output, path_str);
}
