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
