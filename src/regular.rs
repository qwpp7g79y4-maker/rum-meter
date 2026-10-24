//! Read a regular file without blocking on a FIFO.
//!
//! The file is opened first. On Unix the open sets `O_NONBLOCK`, then the type
//! is taken from that descriptor (`fstat`), and the bytes are read from the
//! same descriptor. A directory, a FIFO, a device, or a symlink to one of
//! those is refused before any byte is read. A symlink to a regular file is
//! followed and read. A path that becomes a FIFO between the name and the
//! open cannot block: the open returns, and the descriptor is not a regular
//! file.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Read};
use std::path::Path;

#[derive(Debug)]
pub(crate) enum RegularError {
    Unreadable { kind: &'static str },
    NotRegular,
}

/// Bytes of a regular file. Nothing is read when the opened file is not one.
pub(crate) fn read_regular(path: &Path) -> Result<Vec<u8>, RegularError> {
    let mut file = open_regular(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| RegularError::Unreadable {
            kind: io_kind_label(err.kind()),
        })?;
    Ok(bytes)
}

/// Open for reading, and keep the file only when the opened descriptor is a
/// regular file. The type is not looked up by path a second time.
#[cfg(unix)]
fn open_regular(path: &Path) -> Result<File, RegularError> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    match OpenOptions::new()
        .read(true)
        .custom_flags(o_nonblock())
        .open(path)
    {
        Ok(file) => match file.metadata() {
            Ok(meta) if meta.is_file() => Ok(file),
            Ok(_) => Err(RegularError::NotRegular),
            Err(err) => Err(unreadable(err)),
        },
        // A directory or a socket fails at open. A FIFO does not: `O_NONBLOCK`
        // makes that open return, and the descriptor check above refuses it.
        // When open failed, classify a non-file from its metadata. `stat`
        // does not block on a FIFO.
        Err(err) => match fs::metadata(path) {
            Ok(meta) if !meta.is_file() => Err(RegularError::NotRegular),
            _ => Err(unreadable(err)),
        },
    }
}

/// `O_NONBLOCK`. Linux and Android use octal 04000. The BSDs and macOS use `0x4`.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn o_nonblock() -> i32 {
    0o4000
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "android")),))]
fn o_nonblock() -> i32 {
    0x4
}

#[cfg(not(unix))]
fn open_regular(path: &Path) -> Result<File, RegularError> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => File::open(path).map_err(unreadable),
        Ok(_) => Err(RegularError::NotRegular),
        Err(err) => Err(unreadable(err)),
    }
}

/// Bytes from stdin for the state path `-`.
///
/// A pipe or a regular file is read to the end. A terminal, a device, or any
/// other kind is [`RegularError::NotRegular`] and nothing is read. A path that
/// is a FIFO is still refused by [`read_regular`]; this function is only for
/// the literal argument `-`.
pub(crate) fn read_stdin() -> Result<Vec<u8>, RegularError> {
    if !stdin_is_pipe_or_file()? {
        return Err(RegularError::NotRegular);
    }
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .read_to_end(&mut bytes)
        .map_err(|err| RegularError::Unreadable {
            kind: io_kind_label(err.kind()),
        })?;
    Ok(bytes)
}

/// `true` when stdin is a pipe or a regular file. A character device is `false`.
#[cfg(unix)]
fn stdin_is_pipe_or_file() -> Result<bool, RegularError> {
    use std::mem::ManuallyDrop;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::FileTypeExt;

    let fd = io::stdin().as_raw_fd();
    // SAFETY: `fd` is this process's stdin and must stay open. `ManuallyDrop`
    // keeps the wrapper from closing it when the metadata check returns.
    let file = ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
    match file.metadata() {
        Ok(meta) => Ok(meta.file_type().is_fifo() || meta.is_file()),
        Err(err) => Err(unreadable(err)),
    }
}

/// `true` when stdin is a pipe or a disk file. A character device is `false`.
#[cfg(windows)]
fn stdin_is_pipe_or_file() -> Result<bool, RegularError> {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileType(handle: *mut std::ffi::c_void) -> u32;
    }

    const FILE_TYPE_DISK: u32 = 1;
    const FILE_TYPE_PIPE: u32 = 3;

    let handle = io::stdin().as_raw_handle();
    // SAFETY: `handle` is this process's stdin. `GetFileType` only queries it.
    let kind = unsafe { GetFileType(handle) };
    Ok(kind == FILE_TYPE_DISK || kind == FILE_TYPE_PIPE)
}

/// Platforms other than Unix and Windows have no stdin type check here.
#[cfg(not(any(unix, windows)))]
fn stdin_is_pipe_or_file() -> Result<bool, RegularError> {
    Ok(false)
}

fn unreadable(err: io::Error) -> RegularError {
    RegularError::Unreadable {
        kind: io_kind_label(err.kind()),
    }
}

fn io_kind_label(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "not found",
        ErrorKind::PermissionDenied => "permission denied",
        ErrorKind::IsADirectory => "is a directory",
        _ => "io error",
    }
}
