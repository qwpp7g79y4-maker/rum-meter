//! Only a regular file is read, for a state and for `--prices`.
//!
//! `metadata().is_file()` runs before `fs::read`. A FIFO with no writer, a
//! directory, `/dev/zero` and `/dev/null` are exit 2. Each run has a timeout
//! so a missing check cannot hang the suite, and the child is reaped.
//! FIFOs and `/dev` exist only on Unix; the directory check runs on every OS.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn fixture(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .into_os_string()
        .into_string()
        .expect("fixture path is utf-8")
}

fn assert_pid_gone(pid: u32) {
    if !cfg!(target_os = "linux") {
        return;
    }
    assert!(
        !PathBuf::from(format!("/proc/{pid}")).exists(),
        "rum-meter pid {pid} is still running"
    );
}

/// Spawn the binary and give up after one second. The child is killed and reaped.
fn run_bounded(args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rum-meter"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rum-meter");
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(1);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                assert_pid_gone(pid);
                panic!("rum-meter did not exit within 1s: {args:?}");
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                assert_pid_gone(pid);
                panic!("wait: {err}");
            }
        }
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_end(&mut stdout).expect("stdout");
    }
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_end(&mut stderr).expect("stderr");
    }
    let _ = child.wait();
    assert_pid_gone(pid);
    Output {
        status,
        stdout,
        stderr,
    }
}

fn assert_not_regular(label: &str, path: &str, output: &Output) {
    let stderr = String::from_utf8(output.stderr.clone()).expect("stderr is utf-8");
    assert_eq!(
        output.status.code(),
        Some(2),
        "{label} {path} should exit 2\n{stderr}"
    );
    assert!(
        output.stdout.is_empty(),
        "{label} {path} stdout should be empty"
    );
    assert_eq!(
        stderr,
        format!("rum-meter: {path}: not a regular file\n"),
        "{label}"
    );
}

fn temp_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("regular-file");
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

#[cfg(unix)]
fn make_fifo() -> String {
    let path = temp_dir().join("no-writer.fifo");
    let _ = fs::remove_file(&path);
    let status = Command::new("mkfifo").arg(&path).status().expect("mkfifo");
    assert!(status.success(), "mkfifo failed");
    path.into_os_string().into_string().expect("utf-8 path")
}

fn make_dir() -> String {
    let path = temp_dir().join("a-directory");
    fs::create_dir_all(&path).expect("temp directory");
    path.into_os_string().into_string().expect("utf-8 path")
}

#[test]
fn a_directory_as_state_or_prices_exits_2_on_every_os() {
    let good_state = fixture("v4_one_managed.json");
    let dir = make_dir();
    let as_state = run_bounded(&["count", &dir]);
    assert_not_regular("state", &dir, &as_state);
    let as_prices = run_bounded(&["count", "--prices", &dir, &good_state]);
    assert_not_regular("prices", &dir, &as_prices);
}

#[cfg(unix)]
#[test]
fn non_regular_state_and_price_paths_exit_2_without_reading() {
    let good_state = fixture("v4_one_managed.json");
    let good_prices = fixture("example-prices.json");
    let fifo = make_fifo();
    let dir = make_dir();
    let paths = ["/dev/zero", fifo.as_str(), dir.as_str(), "/dev/null"];
    for path in paths {
        let as_state = run_bounded(&["count", path]);
        assert_not_regular("state", path, &as_state);
        let as_prices = run_bounded(&["count", "--prices", path, &good_state]);
        assert_not_regular("prices", path, &as_prices);
    }

    let ok = run_bounded(&["count", "--prices", &good_prices, &good_state]);
    assert_eq!(
        ok.status.code(),
        Some(0),
        "a regular file should still count"
    );
    let stdout = String::from_utf8(ok.stdout).expect("utf-8");
    assert!(stdout.contains("total rum: 1\n"), "{stdout}");
    assert!(stdout.contains("example-a: EUR 730.00\n"), "{stdout}");
    assert!(
        ok.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}
