//! Read a regular file without blocking on a FIFO.
//!
//! The file is opened first. On Unix the open sets `O_NONBLOCK`, then the type
//! is taken from that descriptor (`fstat`), and the bytes are read from the
//! same descriptor. A directory, a FIFO, a device, or a symlink to one of
//! those is refused before any byte is read. A symlink to a regular file is
//! followed and read. A path that becomes a FIFO between the name and the
//! open cannot block: the open returns, and the descriptor is not a regular
//! file.
//!
//! A regular file whose size is already over [`MAX_INPUT_BYTES`] is refused
//! from its metadata, before any byte is read. Reading still stops as soon as
//! the total would exceed the cap, so a pipe, or a file that grows after the
//! size check, cannot be read past that cap either.

use std::fs::{self, File};
use std::io::{self, ErrorKind, Read};
use std::path::Path;

#[derive(Debug)]
pub(crate) enum RegularError {
    Unreadable {
        kind: &'static str,
    },
    NotRegular,
    /// More than [`MAX_INPUT_BYTES`] were read. Nothing further was read.
    TooLarge,
}

/// Logical size is a lower bound, not an upper one: a sparse file over this
/// size is refused from `stat` alone, and a file under it may still grow, so
/// the cap is also enforced on the bytes actually read, one chunk at a time.
pub(crate) const MAX_INPUT_BYTES: u64 = 128 * 1024 * 1024;

/// Bytes of a regular file. Nothing is read when the opened file is not one.
pub(crate) fn read_regular(path: &Path) -> Result<Vec<u8>, RegularError> {
    let (file, len) = open_regular(path)?;
    read_capped(file, Some(len))
}

/// Read until EOF or until the total would exceed [`MAX_INPUT_BYTES`], in which
/// case nothing more is read and the bytes collected so far are discarded.
///
/// `len` is the number of bytes left before EOF when the reader is a regular
/// file. Over the cap, the reader is refused without a single read: reading a
/// sparse file's 128 MiB of zeros just to throw them away is slow on some
/// runners.
fn read_capped(mut reader: impl Read, len: Option<u64>) -> Result<Vec<u8>, RegularError> {
    if len.is_some_and(|len| len > MAX_INPUT_BYTES) {
        return Err(RegularError::TooLarge);
    }
    let mut bytes = Vec::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(unreadable)?;
        if n == 0 {
            return Ok(bytes);
        }
        if bytes.len() as u64 + n as u64 > MAX_INPUT_BYTES {
            return Err(RegularError::TooLarge);
        }
        bytes.extend_from_slice(&buf[..n]);
    }
}

/// Open for reading, and keep the file and its size only when the opened
/// descriptor is a regular file. The type is not looked up by path a second
/// time.
#[cfg(unix)]
fn open_regular(path: &Path) -> Result<(File, u64), RegularError> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    match OpenOptions::new()
        .read(true)
        .custom_flags(o_nonblock())
        .open(path)
    {
        Ok(file) => match file.metadata() {
            Ok(meta) if meta.is_file() => Ok((file, meta.len())),
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
fn open_regular(path: &Path) -> Result<(File, u64), RegularError> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => {
            let file = File::open(path).map_err(unreadable)?;
            let len = file.metadata().map_err(unreadable)?.len();
            Ok((file, len))
        }
        Ok(_) => Err(RegularError::NotRegular),
        Err(err) => Err(unreadable(err)),
    }
}

/// Bytes from stdin for the state path `-`.
///
/// A pipe or a regular file is read up to [`MAX_INPUT_BYTES`]. A regular file
/// with more than that left to read is refused before any byte is read. A
/// terminal, a device, or any other kind is [`RegularError::NotRegular`] and
/// nothing is read. A path that is a FIFO is still refused by
/// [`read_regular`]; this function is only for the literal argument `-`.
pub(crate) fn read_stdin() -> Result<Vec<u8>, RegularError> {
    let Some(len) = stdin_kind()? else {
        return Err(RegularError::NotRegular);
    };
    read_capped(io::stdin().lock(), len)
}

/// `None` when stdin is neither a pipe nor a regular file, such as a
/// character device. `Some(None)` for a pipe. `Some(Some(n))` for a regular
/// file with `n` bytes between the current offset and its end.
#[cfg(unix)]
fn stdin_kind() -> Result<Option<Option<u64>>, RegularError> {
    use std::io::Seek;
    use std::mem::ManuallyDrop;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::FileTypeExt;

    let fd = io::stdin().as_raw_fd();
    // SAFETY: `fd` is this process's stdin and must stay open. `ManuallyDrop`
    // keeps the wrapper from closing it when the metadata check returns.
    let file = ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
    let meta = file.metadata().map_err(unreadable)?;
    if meta.file_type().is_fifo() {
        return Ok(Some(None));
    }
    if !meta.is_file() {
        return Ok(None);
    }
    // A redirected file can arrive part-read. Without an offset, the read
    // loop alone enforces the cap.
    let left = (&*file)
        .stream_position()
        .ok()
        .map(|pos| meta.len().saturating_sub(pos));
    Ok(Some(left))
}

/// `None` when stdin is neither a pipe nor a disk file, such as a console.
/// A disk file gets no size check here; the read loop enforces the cap.
#[cfg(windows)]
fn stdin_kind() -> Result<Option<Option<u64>>, RegularError> {
    use std::os::windows::io::AsRawHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileType(handle: *mut std::ffi::c_void) -> u32;
    }

    const FILE_TYPE_DISK: u32 = 1;
    const FILE_TYPE_PIPE: u32 = 3;

    let handle = io::stdin().as_raw_handle();
    // SAFETY: `handle` is this process's stdin. `GetFileType` only queries it.
    let kind = unsafe { GetFileType(handle) };
    Ok((kind == FILE_TYPE_DISK || kind == FILE_TYPE_PIPE).then_some(None))
}

/// Platforms other than Unix and Windows have no stdin type check here.
#[cfg(not(any(unix, windows)))]
fn stdin_kind() -> Result<Option<Option<u64>>, RegularError> {
    Ok(None)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Endless zeros that count every byte handed out.
    struct CountingZeros {
        read: u64,
    }

    impl Read for CountingZeros {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            buf.fill(0);
            self.read += buf.len() as u64;
            Ok(buf.len())
        }
    }

    #[test]
    fn a_known_length_over_the_cap_is_refused_before_any_read() {
        let mut reader = CountingZeros { read: 0 };
        let result = read_capped(&mut reader, Some(MAX_INPUT_BYTES + 1));
        assert!(matches!(result, Err(RegularError::TooLarge)));
        assert_eq!(reader.read, 0, "no byte should be read");
    }

    #[test]
    fn a_known_length_at_the_cap_is_read() {
        let mut reader = io::repeat(b'x').take(4);
        let bytes = read_capped(&mut reader, Some(MAX_INPUT_BYTES)).expect("read");
        assert_eq!(bytes, b"xxxx");
    }

    #[test]
    fn an_unknown_length_stops_one_chunk_past_the_cap() {
        let mut reader = CountingZeros { read: 0 };
        let result = read_capped(&mut reader, None);
        assert!(matches!(result, Err(RegularError::TooLarge)));
        assert!(reader.read <= MAX_INPUT_BYTES + 64 * 1024);
    }
}
