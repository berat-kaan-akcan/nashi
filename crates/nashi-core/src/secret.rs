//! A password that never reaches an argument list, a log or a debug print.
//!
//! `nashi-cli --password-stdin` reads it from standard input and the engine
//! hands it to 7z or bsdtar the same way, so it never shows up in
//! `/proc/PID/cmdline` (readable by every user) or in the environment.

use std::fmt;
use std::io::Read;

use crate::events::{JobError, JobResult};

/// Longest password accepted, in bytes.
pub const MAX_LEN: usize = 4096;

/// Password bytes, overwritten with zeros when dropped.
///
/// Best effort: the bytes are wiped here, but a copy the kernel or the reading
/// process made on the way in is out of reach.
pub struct Secret(Vec<u8>);

impl Secret {
    /// `None` for an empty password: an empty box means "no password".
    pub fn new(bytes: Vec<u8>) -> Option<Secret> {
        if bytes.is_empty() {
            return None;
        }
        // Never shrunk or grown: a reallocation would leave an unwiped copy.
        Some(Secret(bytes))
    }

    /// One line from `r`: up to [`MAX_LEN`] bytes, without its `\n` or `\r\n`.
    /// Reads straight into its own buffer, so give it an unbuffered reader
    /// (see [`Secret::read_stdin`]) to keep the bytes out of other buffers.
    pub fn read_line(r: &mut impl Read) -> JobResult<Option<Secret>> {
        // Allocated once at full size and filled in place: never reallocated.
        let mut raw = Secret(vec![0u8; MAX_LEN + 2]);
        let mut len = 0;
        while len < raw.0.len() && !raw.0[..len].contains(&b'\n') {
            match r.read(&mut raw.0[len..]) {
                Ok(0) => break,
                Ok(n) => len += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => {
                    return Err(JobError::invalid_options(format!(
                        "could not read the password: {e}"
                    )))
                }
            }
        }
        let mut end = raw.0[..len].iter().position(|b| *b == b'\n').unwrap_or(len);
        if end > 0 && raw.0[end - 1] == b'\r' {
            end -= 1;
        }
        if end > MAX_LEN {
            return Err(JobError::invalid_options(format!(
                "the password is longer than {MAX_LEN} bytes"
            )));
        }
        if raw.0[..end].contains(&0) {
            return Err(JobError::invalid_options(
                "the password contains a NUL byte",
            ));
        }
        wipe(&mut raw.0[end..]);
        raw.0.truncate(end);
        Ok(if end == 0 { None } else { Some(raw) })
    }

    /// [`Secret::read_line`] on file descriptor 0, bypassing `std::io::stdin()`'s buffer.
    pub fn read_stdin() -> JobResult<Option<Secret>> {
        use std::os::fd::FromRawFd;
        // SAFETY: fd 0 stays open for the life of the process, and ManuallyDrop
        // keeps this File from closing it.
        let mut fd0 = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(0) });
        Secret::read_line(&mut *fd0)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

fn wipe(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        // SAFETY: `b` is a valid, aligned, exclusive reference.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

impl Drop for Secret {
    fn drop(&mut self) {
        let len = self.0.capacity();
        // Wipe the whole allocation, including bytes past `len` cut off above.
        // SAFETY: the capacity is allocated, and u8 has no invalid values.
        unsafe { self.0.set_len(len) };
        wipe(&mut self.0);
    }
}

impl Clone for Secret {
    fn clone(&self) -> Secret {
        let mut v = Vec::with_capacity(self.0.len());
        v.extend_from_slice(&self.0);
        Secret(v)
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<hidden>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(input: &[u8]) -> JobResult<Option<Secret>> {
        Secret::read_line(&mut &input[..])
    }

    #[test]
    fn reads_one_line() {
        assert_eq!(
            line(b"p@ss word\n").unwrap().unwrap().expose(),
            b"p@ss word"
        );
        assert_eq!(line(b"crlf\r\nignored").unwrap().unwrap().expose(), b"crlf");
        assert_eq!(
            line(b"no newline").unwrap().unwrap().expose(),
            b"no newline"
        );
        assert_eq!(
            line("şifre\n".as_bytes()).unwrap().unwrap().expose(),
            "şifre".as_bytes()
        );
    }

    #[test]
    fn empty_is_no_password() {
        assert!(line(b"").unwrap().is_none());
        assert!(line(b"\n").unwrap().is_none());
    }

    #[test]
    fn refuses_long_or_nul() {
        assert!(line(&vec![b'a'; MAX_LEN]).unwrap().is_some());
        assert!(line(&vec![b'a'; MAX_LEN + 1]).is_err());
        assert!(line(b"a\0b\n").is_err());
    }

    #[test]
    fn debug_hides_it() {
        let s = line(b"hunter2").unwrap().unwrap();
        assert!(!format!("{s:?}").contains("hunter2"));
    }
}
