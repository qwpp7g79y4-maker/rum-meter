//! The state path `-` reads stdin. A real path is still a regular file only.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rum-meter"))
}

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .into_os_string()
        .into_string()
        .expect("fixture path is utf-8")
}

fn piped(args: &[&str], bytes: &[u8]) -> Output {
    let mut child = bin()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rum-meter");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(bytes)
        .expect("write stdin");
    child.wait_with_output().expect("wait rum-meter")
}

/// Same report and exit code as the file path. The report names the path it
/// was given, so `-` stands where the file path stood.
#[test]
fn dash_matches_the_file_except_the_path() {
    let path = fixture("v4_one_managed.json");
    let bytes = std::fs::read(&path).expect("read fixture");
    let from_file = bin().args(["count", &path]).output().expect("count file");
    let from_stdin = piped(&["count", "-"], &bytes);
    assert_eq!(from_file.status.code(), Some(0));
    assert_eq!(from_stdin.status.code(), from_file.status.code());
    assert!(from_stdin.stderr.is_empty());
    let file_stdout = String::from_utf8(from_file.stdout).expect("utf-8");
    let stdin_stdout = String::from_utf8(from_stdin.stdout).expect("utf-8");
    assert_eq!(file_stdout.replace(&path, "-"), stdin_stdout);
    assert!(stdin_stdout.contains("file: -\n"));
    assert!(stdin_stdout.contains("rum: 1\n"));
}

#[test]
fn diff_dash_matches_the_file_except_the_path() {
    let old = fixture("v4_one_managed.json");
    let new = fixture("v4_handwritten.json");
    let bytes = std::fs::read(&new).expect("read fixture");
    let from_file = bin()
        .args(["diff", &old, &new])
        .output()
        .expect("diff files");
    let from_stdin = piped(&["diff", &old, "-"], &bytes);
    assert_eq!(from_file.status.code(), Some(0));
    assert_eq!(from_stdin.status.code(), from_file.status.code());
    assert!(from_stdin.stderr.is_empty());
    let file_stdout = String::from_utf8(from_file.stdout).expect("utf-8");
    let stdin_stdout = String::from_utf8(from_stdin.stdout).expect("utf-8");
    assert_eq!(file_stdout.replace(&new, "-"), stdin_stdout);
}

#[cfg(unix)]
#[test]
fn dash_on_a_device_is_refused() {
    let devnull = File::open("/dev/null").expect("open /dev/null");
    let output = bin()
        .args(["count", "-"])
        .stdin(devnull)
        .output()
        .expect("count -");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("utf-8");
    assert_eq!(stderr, "rum-meter: -: not a regular file\n");
}

#[test]
fn prices_dash_does_not_read_stdin() {
    let state = fixture("v4_one_managed.json");
    let output = bin()
        .args(["count", "--prices", "-", &state])
        .output()
        .expect("count --prices -");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("utf-8");
    assert_eq!(stderr, "rum-meter: -: unreadable (not found)\n");
}
