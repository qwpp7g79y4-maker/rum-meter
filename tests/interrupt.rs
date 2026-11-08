//! SIGINT exits 130. The signal goes only to the child this test started,
//! through a pidfd, so a recycled pid or the test process is not signalled.
//!
//! An `--output` file is not left half-written. A write that fails leaves the
//! previous bytes in place. Exit 1 and exit 2 are not reused.

#![cfg(target_os = "linux")]

use std::fs;
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PRIOR: &[u8] = b"PRIOR-BYTES-NOT-THE-REPORT\n";
/// Linux `x86_64` and `aarch64` use the generic syscall numbers.
const SYS_PIDFD_SEND_SIGNAL: isize = 424;
const SYS_PIDFD_OPEN: isize = 434;
const SIGINT: i32 = 2;
const SIGXFSZ: i32 = 25;
const RLIMIT_FSIZE: i32 = 1;
const SIG_IGN: *const () = std::ptr::without_provenance(1);
const SIG_ERR: *const () = std::ptr::without_provenance(usize::MAX);

unsafe extern "C" {
    fn syscall(number: isize, ...) -> isize;
    fn signal(signum: i32, handler: *const ()) -> *const ();
    fn setrlimit(resource: i32, limit: *const RLimit) -> i32;
    fn __errno_location() -> *mut i32;
}

#[repr(C)]
struct RLimit {
    rlim_cur: u64,
    rlim_max: u64,
}

fn scratch() -> PathBuf {
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "rum-meter-sigint-{}-{}-{}",
        std::process::id(),
        nanos,
        N.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&dir).expect("create scratch dir");
    dir
}

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

struct ChildGuard(Option<Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct ScratchGuard(PathBuf);

impl Drop for ScratchGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn blocked_on_stdin(text: &str) -> bool {
    let mut fields = text.split_whitespace();
    let number = fields.next();
    let fd = fields.next();
    number == Some("0") && matches!(fd, Some("0x0" | "0"))
}

fn wait_until_stdin_blocks(child: &mut Child) {
    let pid = child.id();
    let start = Instant::now();
    let limit = Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            panic!("child exited before SIGINT: {status:?}");
        }
        let text = fs::read_to_string(format!("/proc/{pid}/syscall")).unwrap_or_default();
        if blocked_on_stdin(&text) {
            return;
        }
        if start.elapsed() >= limit {
            panic!("child {pid} did not block reading stdin within {limit:?}: {text:?}");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for_exit(child: &mut Child) -> ExitStatus {
    let start = Instant::now();
    let limit = Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().expect("wait") {
            return status;
        }
        if start.elapsed() >= limit {
            panic!("child stayed alive after the signal");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

/// Open a pidfd for `pid` and signal only that process.
fn signal_child(pid: u32) -> OwnedFd {
    assert_ne!(pid, std::process::id(), "the test must not signal itself");
    let pid = i32::try_from(pid).expect("pid fits");
    // SAFETY: `pidfd_open` returns a new descriptor for this pid, or -1.
    let opened = unsafe { syscall(SYS_PIDFD_OPEN, pid as isize, 0isize) };
    assert!(
        opened >= 0,
        "pidfd_open({pid}) failed: {}",
        std::io::Error::last_os_error()
    );
    let fd = i32::try_from(opened).expect("fd fits");
    // SAFETY: `fd` is the descriptor `pidfd_open` just returned.
    let pidfd = unsafe { OwnedFd::from_raw_fd(fd) };
    // Flags 0. `PIDFD_SIGNAL_PROCESS_GROUP` is not set, so the test process,
    // which shares the child's group, is not signalled. A plain pid would
    // also miss a reused pid; the pidfd does not.
    // SAFETY: the descriptor refers to the child this test spawned.
    let sent = unsafe {
        syscall(
            SYS_PIDFD_SEND_SIGNAL,
            pidfd.as_raw_fd() as isize,
            SIGINT as isize,
            0isize,
            0isize,
        )
    };
    assert!(
        sent == 0,
        "pidfd_send_signal failed: {}",
        std::io::Error::last_os_error()
    );
    pidfd
}

fn only_file(dir: &Path, name: &str) {
    let mut names: Vec<_> = fs::read_dir(dir)
        .expect("read scratch")
        .map(|entry| entry.expect("dir entry").file_name())
        .collect();
    names.sort();
    assert_eq!(names, vec![std::ffi::OsString::from(name)]);
}

#[test]
fn sigint_exits_130_and_leaves_the_report_intact() {
    let root = ScratchGuard(scratch());
    let report = root.0.join("report.txt");
    fs::write(&report, PRIOR).unwrap();
    let report_path = report.to_str().expect("utf-8 path");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.args(["count", "--output", report_path, "-"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut guard = ChildGuard(Some(cmd.spawn().expect("spawn rum-meter")));
    let child = guard.0.as_mut().expect("child");
    let pid = child.id();
    wait_until_stdin_blocks(child);
    let _pidfd = signal_child(pid);
    let status = wait_for_exit(child);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(out) = child.stdout.as_mut() {
        out.read_to_end(&mut stdout).expect("stdout");
    }
    if let Some(err) = child.stderr.as_mut() {
        err.read_to_end(&mut stderr).expect("stderr");
    }
    guard.0.take();

    assert_eq!(status.code(), Some(130), "stderr={stderr:?}");
    assert_ne!(status.code(), Some(0));
    assert_ne!(status.code(), Some(1), "a threshold was not renumbered");
    assert_ne!(status.code(), Some(2), "an input error was not renumbered");
    assert!(stdout.is_empty(), "{stdout:?}");
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(fs::read(&report).expect("read report"), PRIOR);
    only_file(&root.0, "report.txt");
}

#[test]
fn a_failed_report_write_keeps_the_previous_bytes() {
    let root = ScratchGuard(scratch());
    let report = root.0.join("report.txt");
    fs::write(&report, PRIOR).unwrap();
    let report_path = report.to_str().expect("utf-8 path");
    let state = fixture("v4_one_managed.json");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.args(["count", "--output", report_path, &state])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: the closure runs after fork. It only calls `signal` and
    // `setrlimit`, and builds the error from the errno integer.
    unsafe {
        cmd.pre_exec(|| {
            let limit = RLimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            let previous = signal(SIGXFSZ, SIG_IGN);
            if previous == SIG_ERR {
                return Err(std::io::Error::from_raw_os_error(*__errno_location()));
            }
            if setrlimit(RLIMIT_FSIZE, &limit) != 0 {
                return Err(std::io::Error::from_raw_os_error(*__errno_location()));
            }
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn rum-meter");
    let status = wait_for_exit(&mut child);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(out) = child.stdout.as_mut() {
        out.read_to_end(&mut stdout).expect("stdout");
    }
    if let Some(err) = child.stderr.as_mut() {
        err.read_to_end(&mut stderr).expect("stderr");
    }
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    assert_eq!(status.code(), Some(2), "{stderr}");
    assert_ne!(status.code(), Some(1), "a failed write is not a threshold");
    assert!(stdout.is_empty(), "{stdout:?}");
    assert!(stderr.contains("cannot write report to "), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert_eq!(fs::read(&report).expect("read report"), PRIOR);
    only_file(&root.0, "report.txt");
}
