//! Only a regular file is read, for a state and for `--prices`.
//!
//! A FIFO with no writer, a symlink to a FIFO, a directory, `/dev/zero` and
//! `/dev/null` are exit 2. Each run has a timeout so a missing check cannot
//! hang the suite, and the child is reaped. FIFOs and `/dev` exist only on
//! Unix; the directory check runs on every OS.

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

/// A symlink to a FIFO is the FIFO. The open follows the link and exits 2
/// without waiting for a writer.
#[cfg(unix)]
#[test]
fn symlink_to_fifo_exits_2_within_2s() {
    let good_state = fixture("v4_one_managed.json");
    let dir = temp_dir();
    let fifo = dir.join("linked.fifo");
    let _ = fs::remove_file(&fifo);
    let status = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(status.success(), "mkfifo failed");
    let link = dir.join("state-link");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(&fifo, &link).expect("symlink");
    let path = link.into_os_string().into_string().expect("utf-8 path");
    let started = Instant::now();
    let as_state = run_bounded(&["count", &path]);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "symlink to a FIFO took {:?}",
        started.elapsed()
    );
    assert_not_regular("state", &path, &as_state);
    let price_link = dir.join("price-link");
    let _ = fs::remove_file(&price_link);
    std::os::unix::fs::symlink(&fifo, &price_link).expect("symlink");
    let price_path = price_link
        .into_os_string()
        .into_string()
        .expect("utf-8 path");
    let as_prices = run_bounded(&["count", "--prices", &price_path, &good_state]);
    assert_not_regular("prices", &price_path, &as_prices);
}

/// A symlink to a regular file is that file.
#[cfg(unix)]
#[test]
fn symlink_to_regular_file_counts() {
    let good_state = fixture("v4_one_managed.json");
    let good_prices = fixture("example-prices.json");
    let dir = temp_dir();
    let state_link = dir.join("real-state");
    let price_link = dir.join("real-prices");
    let _ = fs::remove_file(&state_link);
    let _ = fs::remove_file(&price_link);
    std::os::unix::fs::symlink(&good_state, &state_link).expect("symlink");
    std::os::unix::fs::symlink(&good_prices, &price_link).expect("symlink");
    let state = state_link.into_os_string().into_string().expect("utf-8");
    let prices = price_link.into_os_string().into_string().expect("utf-8");
    let output = run_bounded(&["count", "--prices", &prices, &state]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(stdout.contains("total rum: 1\n"), "{stdout}");
    assert!(stdout.contains("example-a: EUR 730.00\n"), "{stdout}");
}

/// The path is a regular file at the name and a symlink to a FIFO at the open.
/// The process must exit 2 within 2 seconds, not block in `open`.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn symlink_becomes_fifo_between_name_and_open_exits_2() {
    let dir = temp_dir().join("race");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("race dir");
    let real = dir.join("real.json");
    fs::write(
        &real,
        r#"{"version":4,"resources":[{"mode":"managed","type":"random_id","name":"x","instances":[{}]}]}"#,
    )
    .expect("state");
    let fifo = dir.join("pipe");
    let status = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(status.success(), "mkfifo failed");
    let link = dir.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink to file");
    let fifo_link = dir.join("link-fifo");
    std::os::unix::fs::symlink(&fifo, &fifo_link).expect("symlink to fifo");
    use std::os::unix::ffi::OsStrExt;
    let link_bytes = link.as_os_str().as_bytes().to_vec();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rum-meter"));
    cmd.arg("count")
        .arg(&link)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: `traceme` only issues the ptrace syscall. It runs after fork and
    // does not allocate or touch other memory.
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(trace::traceme);
    }
    let mut guard = ChildGuard(Some(cmd.spawn().expect("spawn rum-meter")));
    let pid = i32::try_from(guard.0.as_ref().expect("child").id()).expect("pid fits");
    let start = Instant::now();
    let limit = Duration::from_secs(2);
    let status = trace::wait_stop(pid).expect("exec stop");
    assert!(
        trace::is_stopped(status),
        "child did not stop after exec: {status:#x}"
    );
    trace::ptrace(
        trace::PTRACE_SETOPTIONS,
        pid,
        0,
        i64::from(trace::PTRACE_O_TRACESYSGOOD),
    )
    .expect("set options");
    trace::ptrace(trace::PTRACE_SYSCALL, pid, 0, 0).expect("resume");

    let mut entering = true;
    let mut swapped = false;
    let code = loop {
        if start.elapsed() >= limit {
            panic!("timeout after {limit:?}, the open waited on the FIFO");
        }
        let status = match trace::wait_flags(pid, trace::WNOHANG) {
            Ok(Some(status)) => status,
            Ok(None) => {
                thread::sleep(Duration::from_millis(5));
                continue;
            }
            Err(err) => panic!("wait: {err}"),
        };
        if let Some(code) = trace::exit_code(status) {
            break code;
        }
        if trace::stop_signal(status) == Some(trace::SIGTRAP | 0x80) {
            if entering && !swapped && trace::open_path_is(pid, &link_bytes) {
                fs::rename(&fifo_link, &link).expect("swap symlink to fifo");
                swapped = true;
            }
            entering = !entering;
            trace::ptrace(trace::PTRACE_SYSCALL, pid, 0, 0).expect("resume syscall");
        } else if let Some(sig) = trace::stop_signal(status) {
            let deliver = if sig == trace::SIGTRAP {
                0
            } else {
                i64::from(sig)
            };
            trace::ptrace(trace::PTRACE_SYSCALL, pid, 0, deliver).expect("resume signal");
        }
    };
    let mut child = guard.0.take().expect("child");
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    // The tracer already reaped the child. Drop would signal that pid.
    #[allow(clippy::mem_forget)]
    std::mem::forget(child);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(out) = stdout_pipe.as_mut() {
        out.read_to_end(&mut stdout).expect("stdout");
    }
    if let Some(err) = stderr_pipe.as_mut() {
        err.read_to_end(&mut stderr).expect("stderr");
    }
    assert!(swapped, "the open of the symlink was not seen");
    assert!(
        start.elapsed() < limit,
        "symlink to a FIFO took {:?}",
        start.elapsed()
    );
    assert_eq!(code, 2, "stdout={stdout:?} stderr={stderr:?}");
    assert!(stdout.is_empty(), "{stdout:?}");
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    let shown = link.display().to_string();
    assert!(
        stderr.contains(&format!("{shown}:")),
        "the message does not name the path\n{stderr}"
    );
    assert!(
        stderr.contains("not a regular file"),
        "the message lacks 'not a regular file':\n{stderr}"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
struct ChildGuard(Option<std::process::Child>);

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// x86_64 Linux ptrace. Syscall numbers and register slots are the kernel ABI.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod trace {
    use std::io::{Read, Seek, SeekFrom};

    pub const PTRACE_SYSCALL: i64 = 24;
    pub const PTRACE_SETOPTIONS: i64 = 0x4200;
    pub const PTRACE_O_TRACESYSGOOD: i32 = 1;
    pub const WNOHANG: i32 = 1;
    pub const SIGTRAP: i32 = 5;
    const PTRACE_TRACEME: i64 = 0;
    const PTRACE_GETREGS: i64 = 12;
    const SYS_OPEN: u64 = 2;
    const SYS_OPENAT: u64 = 257;
    const SYS_OPENAT2: u64 = 437;
    const SYS_PTRACE: i64 = 101;
    const SYS_WAIT4: i64 = 61;
    /// `user_regs_struct.rsi` on x86_64.
    const RSI: usize = 13;
    /// `user_regs_struct.rdi` on x86_64. Pathname argument of `open`.
    const RDI: usize = 14;
    /// `user_regs_struct.orig_rax`: the syscall number.
    const ORIG_RAX: usize = 15;

    pub fn traceme() -> std::io::Result<()> {
        let rc = syscall4(SYS_PTRACE, PTRACE_TRACEME, 0, 0, 0);
        if rc < 0 {
            Err(std::io::Error::from_raw_os_error(errno_code(rc)))
        } else {
            Ok(())
        }
    }

    pub fn ptrace(req: i64, pid: i32, addr: i64, data: i64) -> std::io::Result<i64> {
        let rc = syscall4(SYS_PTRACE, req, i64::from(pid), addr, data);
        if rc < 0 {
            Err(std::io::Error::from_raw_os_error(errno_code(rc)))
        } else {
            Ok(rc)
        }
    }

    pub fn wait_stop(pid: i32) -> std::io::Result<i32> {
        match wait_flags(pid, 0)? {
            Some(status) => Ok(status),
            None => panic!("blocking wait returned no status"),
        }
    }

    pub fn wait_flags(pid: i32, options: i32) -> std::io::Result<Option<i32>> {
        let mut status = 0i32;
        let rc = syscall4(
            SYS_WAIT4,
            i64::from(pid),
            std::ptr::addr_of_mut!(status) as i64,
            i64::from(options),
            0,
        );
        if rc == 0 {
            Ok(None)
        } else if rc < 0 {
            Err(std::io::Error::from_raw_os_error(errno_code(rc)))
        } else {
            Ok(Some(status))
        }
    }

    pub fn is_stopped(status: i32) -> bool {
        status & 0xff == 0x7f
    }

    pub fn stop_signal(status: i32) -> Option<i32> {
        if is_stopped(status) {
            Some((status >> 8) & 0xff)
        } else {
            None
        }
    }

    pub fn exit_code(status: i32) -> Option<i32> {
        if status & 0x7f == 0 {
            Some((status >> 8) & 0xff)
        } else {
            None
        }
    }

    pub fn open_path_is(pid: i32, want: &[u8]) -> bool {
        let mut regs = [0u64; 27];
        if ptrace(PTRACE_GETREGS, pid, 0, regs.as_mut_ptr() as i64).is_err() {
            return false;
        }
        let addr = match regs[ORIG_RAX] {
            SYS_OPEN => regs[RDI],
            SYS_OPENAT | SYS_OPENAT2 => regs[RSI],
            _ => return false,
        };
        read_cstr(pid, addr).as_deref() == Some(want)
    }

    fn read_cstr(pid: i32, addr: u64) -> Option<Vec<u8>> {
        let mut mem = std::fs::File::open(format!("/proc/{pid}/mem")).ok()?;
        mem.seek(SeekFrom::Start(addr)).ok()?;
        let mut buf = [0u8; 4096];
        let n = mem.read(&mut buf).ok()?;
        let end = buf[..n].iter().position(|byte| *byte == 0).unwrap_or(n);
        Some(buf[..end].to_vec())
    }

    fn errno_code(rc: i64) -> i32 {
        i32::try_from(-rc).unwrap_or(i32::MAX)
    }

    fn syscall4(number: i64, a: i64, b: i64, c: i64, d: i64) -> i64 {
        let ret: i64;
        // SAFETY: the numbers are the x86_64 Linux syscall ABI. `rcx` and `r11`
        // are clobbered by `syscall`.
        unsafe {
            std::arch::asm!(
                "syscall",
                inlateout("rax") number => ret,
                in("rdi") a,
                in("rsi") b,
                in("rdx") c,
                in("r10") d,
                lateout("rcx") _,
                lateout("r11") _,
            );
        }
        ret
    }
}
