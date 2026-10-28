//! Output naming and atomic, never-overwriting commits.
//!
//! Every output is first written to `.<name>.nashi-tmp` in the destination
//! directory and then moved to its final, unique name with
//! `renameat2(RENAME_NOREPLACE)` (or `link`+`unlink` where that is not
//! supported), so an existing file is never replaced, even under races.
//! Uncommitted temp files/directories are removed on drop (also on
//! cancellation and errors).

use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// Suffix of every temporary path created by Nashi.
pub const TMP_SUFFIX: &str = ".nashi-tmp";

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn next_id() -> u32 {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// `stem.ext`, `stem (1).ext`, … (`ext` may be empty for directories and
/// may contain dots, e.g. `tar.gz`).
pub fn candidate(dir: &Path, stem: &str, ext: &str, n: u32) -> PathBuf {
    let base = if n == 0 {
        stem.to_string()
    } else {
        format!("{stem} ({n})")
    };
    if ext.is_empty() {
        dir.join(base)
    } else {
        dir.join(format!("{base}.{ext}"))
    }
}

fn exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

/// First free name for `stem.ext` in `dir`.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    (0..)
        .map(|n| candidate(dir, stem, ext, n))
        .find(|p| !exists(p) && !exists(&tmp_for(p)))
        .expect("infinite iterator")
}

fn tmp_for(final_path: &Path) -> PathBuf {
    let name = final_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    final_path.with_file_name(format!(".{name}{TMP_SUFFIX}"))
}

fn cstr(p: &Path) -> io::Result<CString> {
    CString::new(p.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))
}

/// Rename without ever replacing `to`. Returns `AlreadyExists` if taken.
pub fn rename_noreplace(from: &Path, to: &Path) -> io::Result<()> {
    let f = cstr(from)?;
    let t = cstr(to)?;
    // SAFETY: valid NUL-terminated paths; AT_FDCWD with absolute/relative paths.
    let r = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            f.as_ptr(),
            libc::AT_FDCWD,
            t.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if r == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::EOPNOTSUPP) => {
            // Filesystem without RENAME_NOREPLACE.
            if fs::symlink_metadata(from)?.is_dir() {
                fallback_dir(from, to)
            } else {
                match fs::hard_link(from, to) {
                    Ok(()) => fs::remove_file(from),
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
                    Err(_) => fallback_file(from, to),
                }
            }
        }
        _ => Err(err),
    }
}

/// No-replace "rename" of a file by exclusive create + copy (for filesystems
/// without `RENAME_NOREPLACE` and hard links, e.g. some FUSE mounts).
pub fn fallback_file(from: &Path, to: &Path) -> io::Result<()> {
    let mut dst = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    let r = (|| {
        let mut src = fs::File::open(from)?;
        io::copy(&mut src, &mut dst)?;
        dst.sync_all()
    })();
    if let Err(e) = r {
        let _ = fs::remove_file(to);
        return Err(e);
    }
    fs::remove_file(from)
}

/// No-replace "rename" of a directory: `mkdir` the final name (fails if it
/// exists) and move the entries into it.
pub fn fallback_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        fs::rename(e.path(), to.join(e.file_name()))?;
    }
    fs::remove_dir(from)
}

/// A temporary output (file or directory) awaiting commit.
#[derive(Debug)]
pub struct Staged {
    dir: PathBuf,
    stem: String,
    ext: String,
    tmp: PathBuf,
    is_dir: bool,
    committed: bool,
}

impl Staged {
    fn create(dir: &Path, stem: &str, ext: &str, is_dir: bool) -> io::Result<Staged> {
        fs::create_dir_all(dir)?;
        let final_path = unique_path(dir, stem, ext);
        let mut tmp = tmp_for(&final_path);
        loop {
            let r = if is_dir {
                fs::create_dir(&tmp)
            } else {
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&tmp)
                    .map(|_| ())
            };
            match r {
                Ok(()) => break,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    let name = final_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    tmp = final_path.with_file_name(format!(
                        ".{name}.{}-{}{TMP_SUFFIX}",
                        std::process::id(),
                        next_id()
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Staged {
            dir: dir.to_path_buf(),
            stem: stem.to_string(),
            ext: ext.to_string(),
            tmp,
            is_dir,
            committed: false,
        })
    }

    /// Reserve a temp file for the output `dir/stem.ext`.
    pub fn file(dir: &Path, stem: &str, ext: &str) -> io::Result<Staged> {
        Self::create(dir, stem, ext, false)
    }

    /// Reserve a temp directory for the output folder `dir/name/`.
    pub fn dir(dir: &Path, name: &str) -> io::Result<Staged> {
        Self::create(dir, name, "", true)
    }

    /// Temp path the tool should write to.
    pub fn path(&self) -> &Path {
        &self.tmp
    }

    /// Move a finished file (on the same filesystem, e.g. from a [`WorkDir`])
    /// over the reserved temp file.
    pub fn take_from(&self, src: &Path) -> io::Result<()> {
        match fs::rename(src, &self.tmp) {
            Ok(()) => Ok(()),
            Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                fs::copy(src, &self.tmp)?;
                fs::remove_file(src)
            }
            Err(e) => Err(e),
        }
    }

    /// Atomically move into place under a unique name. Never overwrites.
    pub fn commit(self) -> io::Result<PathBuf> {
        self.commit_inner(false)
    }

    /// Like [`Staged::commit`] but accepts an empty file (e.g. text extracted
    /// from a PDF without text).
    pub fn commit_allow_empty(self) -> io::Result<PathBuf> {
        self.commit_inner(true)
    }

    fn commit_inner(mut self, allow_empty: bool) -> io::Result<PathBuf> {
        if !self.is_dir && !allow_empty && fs::metadata(&self.tmp)?.len() == 0 {
            return Err(io::Error::other("tool produced an empty output"));
        }
        let mut n = 0;
        loop {
            let target = candidate(&self.dir, &self.stem, &self.ext, n);
            if exists(&target) {
                n += 1;
                continue;
            }
            match rename_noreplace(&self.tmp, &target) {
                Ok(()) => {
                    self.committed = true;
                    return Ok(target);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => n += 1,
                Err(e) => return Err(e),
            }
        }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.committed {
            let _ = if self.is_dir {
                fs::remove_dir_all(&self.tmp)
            } else {
                fs::remove_file(&self.tmp)
            };
        }
    }
}

/// Private scratch directory, removed on drop. Created inside the destination
/// directory (same filesystem → cheap renames) unless `in_system_tmp`.
#[derive(Debug)]
pub struct WorkDir {
    path: PathBuf,
}

impl WorkDir {
    pub fn new_in(parent: &Path) -> io::Result<WorkDir> {
        fs::create_dir_all(parent)?;
        loop {
            let p = parent.join(format!(
                ".nashi-work-{}-{}{TMP_SUFFIX}",
                std::process::id(),
                next_id()
            ));
            match fs::create_dir(&p) {
                Ok(()) => return Ok(WorkDir { path: p }),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    pub fn system() -> io::Result<WorkDir> {
        Self::new_in(&std::env::temp_dir())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The only regular file directly inside `dir` (tools that pick their own
/// output name, such as LibreOffice).
pub fn single_file_in(dir: &Path) -> io::Result<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    match files.len() {
        1 => Ok(files.remove(0)),
        0 => Err(io::Error::other("tool produced no output")),
        _ => Err(io::Error::other("tool produced several outputs")),
    }
}

/// Sorted regular files in `dir`.
pub fn files_in(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_and_no_overwrite() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.png"), b"x").unwrap();
        fs::write(d.path().join("a (1).png"), b"y").unwrap();
        assert_eq!(
            unique_path(d.path(), "a", "png"),
            d.path().join("a (2).png")
        );
        assert_eq!(
            unique_path(d.path(), "a", "tar.gz"),
            d.path().join("a.tar.gz")
        );

        let s = Staged::file(d.path(), "a", "png").unwrap();
        assert!(s
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(TMP_SUFFIX));
        fs::write(s.path(), b"new").unwrap();
        // Race: someone takes the chosen name before commit.
        fs::write(d.path().join("a (2).png"), b"race").unwrap();
        let out = s.commit().unwrap();
        assert_eq!(out, d.path().join("a (3).png"));
        assert_eq!(fs::read(d.path().join("a.png")).unwrap(), b"x");
        assert_eq!(fs::read(d.path().join("a (2).png")).unwrap(), b"race");
        assert_eq!(fs::read(&out).unwrap(), b"new");
    }

    #[test]
    fn dropped_stage_leaves_nothing() {
        let d = tempfile::tempdir().unwrap();
        {
            let s = Staged::file(d.path(), "b", "mp3").unwrap();
            fs::write(s.path(), b"partial").unwrap();
            let s2 = Staged::dir(d.path(), "b-pages").unwrap();
            fs::write(s2.path().join("x"), b"1").unwrap();
            let _w = WorkDir::new_in(d.path()).unwrap();
        }
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }

    #[test]
    fn empty_output_is_an_error() {
        let d = tempfile::tempdir().unwrap();
        let s = Staged::file(d.path(), "c", "txt").unwrap();
        assert!(s.commit().is_err());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }

    #[test]
    fn no_replace_fallbacks() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        let b = d.path().join("b");
        fs::write(&a, b"new").unwrap();
        fs::write(&b, b"old").unwrap();
        assert_eq!(
            fallback_file(&a, &b).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&b).unwrap(), b"old");
        let c = d.path().join("c");
        fallback_file(&a, &c).unwrap();
        assert_eq!(fs::read(&c).unwrap(), b"new");
        assert!(!a.exists());
        let src = d.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("x"), b"1").unwrap();
        let taken = d.path().join("taken");
        fs::create_dir(&taken).unwrap();
        assert!(fallback_dir(&src, &taken).is_err());
        assert_eq!(fs::read_dir(&taken).unwrap().count(), 0);
        let free = d.path().join("free");
        fallback_dir(&src, &free).unwrap();
        assert_eq!(fs::read(free.join("x")).unwrap(), b"1");
        assert!(!src.exists());
    }

    #[test]
    fn dir_commit() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir(d.path().join("x-pages")).unwrap();
        let s = Staged::dir(d.path(), "x-pages").unwrap();
        fs::write(s.path().join("1.png"), b"1").unwrap();
        assert_eq!(s.commit().unwrap(), d.path().join("x-pages (1)"));
    }
}
