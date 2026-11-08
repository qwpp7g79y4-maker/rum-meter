//! SIGINT stops the process with exit 130.
//!
//! The handler does not return. It unlinks the temporary report, when one is
//! published, and exits. Destructors do not run: a signal handler must not
//! allocate or take a lock. The finished report path is never published, so a
//! previous report stays put and a completed report is not removed.

use std::cell::UnsafeCell;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(unix)]
use std::ffi::c_char;

const SIGINT: i32 = 2;
const EXIT_INTERRUPTED: i32 = 130;

#[cfg(unix)]
const PATH_CAP: usize = 8192;
#[cfg(windows)]
const PATH_CAP: usize = 32768;

/// `ready` orders the bytes. The handler reads them only after an acquire load
/// sees `true`. The writer fills them only while `ready` is false.
struct PartialReport {
    ready: AtomicBool,
    #[cfg(unix)]
    bytes: UnsafeCell<[u8; PATH_CAP]>,
    #[cfg(windows)]
    bytes: UnsafeCell<[u16; PATH_CAP]>,
}

// SAFETY: see the struct comment. The handler does not return into the writer.
unsafe impl Sync for PartialReport {}

#[cfg(any(unix, windows))]
static PARTIAL: PartialReport = PartialReport {
    ready: AtomicBool::new(false),
    bytes: UnsafeCell::new([0; PATH_CAP]),
};

#[cfg(any(unix, windows))]
unsafe extern "C" {
    fn signal(signum: i32, handler: *const ()) -> *const ();
    fn _exit(status: i32) -> !;
}

#[cfg(unix)]
unsafe extern "C" {
    fn unlink(path: *const c_char) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeleteFileW(name: *const u16) -> i32;
    fn MoveFileExW(existing: *const u16, new_name: *const u16, flags: u32) -> i32;
}

#[cfg(windows)]
const MOVEFILE_REPLACE_EXISTING: u32 = 1;

/// Install the SIGINT handler. A failure leaves the default disposition.
pub(crate) fn install() {
    #[cfg(any(unix, windows))]
    {
        // SAFETY: called once from `main` before other threads exist. The
        // handler only unlinks a published path and exits.
        let previous = unsafe { signal(SIGINT, on_sigint as *const ()) };
        let _ = previous;
    }
}

#[cfg(any(unix, windows))]
unsafe extern "C" fn on_sigint(_signum: i32) {
    unlink_partial();
    // SAFETY: `_exit` is async-signal-safe and does not run destructors.
    unsafe { _exit(EXIT_INTERRUPTED) };
}

#[cfg(unix)]
fn unlink_partial() {
    if !PARTIAL.ready.load(Ordering::Acquire) {
        return;
    }
    // SAFETY: `ready` was set after these bytes, including the trailing NUL.
    let bytes = unsafe { &*PARTIAL.bytes.get() };
    // SAFETY: `unlink` only uses that NUL-terminated path.
    unsafe { unlink(bytes.as_ptr().cast()) };
}

#[cfg(windows)]
fn unlink_partial() {
    if !PARTIAL.ready.load(Ordering::Acquire) {
        return;
    }
    // SAFETY: `ready` was set after these UTF-16 units, including the trailing NUL.
    let bytes = unsafe { &*PARTIAL.bytes.get() };
    // SAFETY: `DeleteFileW` only uses that NUL-terminated path.
    unsafe { DeleteFileW(bytes.as_ptr()) };
}

/// Remember `path` so SIGINT can unlink it. The file should not exist yet.
#[cfg(unix)]
pub(crate) fn publish_partial(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    PARTIAL.ready.store(false, Ordering::Release);
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() + 1 > PATH_CAP || bytes.contains(&0) {
        return false;
    }
    // SAFETY: `ready` is false, so the handler is not reading the buffer.
    let slot = unsafe { &mut *PARTIAL.bytes.get() };
    slot[..bytes.len()].copy_from_slice(bytes);
    slot[bytes.len()] = 0;
    PARTIAL.ready.store(true, Ordering::Release);
    true
}

/// Remember `path` so SIGINT can delete it. The file should not exist yet.
#[cfg(windows)]
pub(crate) fn publish_partial(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    PARTIAL.ready.store(false, Ordering::Release);
    let wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.is_empty() || wide.len() + 1 > PATH_CAP || wide.contains(&0) {
        return false;
    }
    // SAFETY: `ready` is false, so the handler is not reading the buffer.
    let slot = unsafe { &mut *PARTIAL.bytes.get() };
    slot[..wide.len()].copy_from_slice(&wide);
    slot[wide.len()] = 0;
    PARTIAL.ready.store(true, Ordering::Release);
    true
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn publish_partial(_path: &Path) -> bool {
    true
}

/// The temporary name is gone. SIGINT must not unlink it.
pub(crate) fn clear_partial() {
    #[cfg(any(unix, windows))]
    PARTIAL.ready.store(false, Ordering::Release);
}

/// Publish `from` as `to`. On Unix the rename replaces `to`.
pub(crate) fn replace_report(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;

        fn wide(path: &Path) -> Vec<u16> {
            path.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        }
        let from_w = wide(from);
        let to_w = wide(to);
        // SAFETY: both buffers are NUL-terminated and live for the call.
        let rc = unsafe { MoveFileExW(from_w.as_ptr(), to_w.as_ptr(), MOVEFILE_REPLACE_EXISTING) };
        if rc == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (from, to);
        Err(std::io::Error::other("unsupported platform"))
    }
}
