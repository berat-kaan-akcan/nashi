//! Archives: 7z, falling back to bsdtar / tar / unzip / gzip.

use std::path::{Path, PathBuf};

use super::{Ctx, Input};
use crate::detect::split_name;
use crate::events::{ErrorCode, JobError, JobResult};
use crate::fsops::WorkDir;
use crate::process::Cmd;

/// A protected archive gets a plain answer instead of the extractor's own
/// words ("Break signaled", "Enter passphrase"), and a code the app answers
/// with a password box.
fn password_protected() -> JobError {
    JobError::new(
        ErrorCode::PasswordRequired,
        "This archive is password protected.",
    )
}

/// Encrypted data cannot tell a wrong key from damage, so the message names both.
fn wrong_password() -> JobError {
    JobError::new(
        ErrorCode::PasswordRequired,
        "The password is incorrect, or the archive is damaged.",
    )
}

/// Whether an extractor failed because the archive asked for a password.
fn asked_for_password(tool: &str, out: &crate::process::RunOut) -> bool {
    let err = &out.stderr;
    match tool {
        // Its password prompt reads the closed stdin and gives up with exit 255.
        "7z" => {
            (out.status.code() == Some(255) && err.contains("Break signaled"))
                || err.contains("Wrong password")
                || err.contains("Cannot open encrypted archive")
        }
        "bsdtar" => {
            err.contains("Enter passphrase")
                || err.contains("Encryption is not supported")
                || err.contains("Passphrase required")
        }
        // 82: nothing extracted for a bad password. Without a terminal it cannot
        // ask and says "unable to get password".
        "unzip" => {
            out.status.code() == Some(82)
                || err.contains("incorrect password")
                || err.contains("unable to get password")
        }
        _ => false,
    }
}

fn extractor_failed(tool: &str, out: &crate::process::RunOut, password: bool) -> JobError {
    if asked_for_password(tool, out) {
        if password {
            wrong_password()
        } else {
            password_protected()
        }
    } else {
        crate::process::failure(tool, out)
    }
}

fn missing_extractor() -> JobError {
    JobError::missing("7z", "7zip")
}

/// The most an archive may expand to. `NASHI_ARCHIVE_LIMIT_BYTES` lowers it for tests.
fn expansion_limit() -> u64 {
    std::env::var("NASHI_ARCHIVE_LIMIT_BYTES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20 * 1024 * 1024 * 1024)
}

fn too_large() -> JobError {
    JobError::invalid_input("The archive expands beyond the safety limit.")
}

/// Decompress a `.gz` stream into `out`.
fn gunzip(ctx: &Ctx, src: &Path, out: &Path) -> JobResult<()> {
    let d = ctx.deps;
    let limit = expansion_limit();
    let f = std::fs::File::create(out)?;
    // One past the limit: a file of exactly the limit is still allowed.
    let result = if let Some(gz) = d.path("gzip") {
        Cmd::new(gz)
            .arg("-dc")
            .arg(src)
            .stdout_to(f)
            .file_limit(limit + 1)
            .run_ok()
    } else if let Some(z) = d.path("7z") {
        Cmd::new(z)
            .args(["e", "-so", "-tgzip"])
            .arg(src)
            .stdout_to(f)
            .file_limit(limit + 1)
            .run_ok()
    } else if let Some(b) = d.path("bsdtar") {
        Cmd::new(b)
            .args(["-xOf"])
            .arg(src)
            .stdout_to(f)
            .file_limit(limit + 1)
            .run_ok()
    } else {
        return Err(missing_extractor());
    };
    if std::fs::metadata(out).is_ok_and(|m| m.len() > limit) {
        return Err(too_large());
    }
    result?;
    Ok(())
}

/// What the archive says it holds, when a tool can list it. The listing can lie,
/// so this only saves the work of extracting an archive that is plainly too large.
fn listed_size(ctx: &Ctx, archive: &Path) -> Option<u64> {
    let d = ctx.deps;
    if let Some(z) = d.path("7z") {
        let mut list = Cmd::new(z).args(["l", "-slt", "--"]).arg(archive);
        // Encrypted names cannot be listed without the password.
        if let Some(pw) = &ctx.opts.password {
            list = list.secret_stdin(pw);
        }
        let out = list.run_ok().ok()?;
        return Some(
            out.stdout
                .lines()
                .filter_map(|l| l.strip_prefix("Size = ")?.trim().parse::<u64>().ok())
                .fold(0, u64::saturating_add),
        );
    }
    let b = d.path("bsdtar")?;
    let out = Cmd::new(b).arg("-tvf").arg(archive).run_ok().ok()?;
    Some(
        out.stdout
            .lines()
            .filter_map(|l| l.split_whitespace().nth(4)?.parse::<u64>().ok())
            .fold(0, u64::saturating_add),
    )
}

/// Stop `cmd` once the files below `dest` pass `limit`. Each file is capped too, but
/// an archive can hold any number of them behind a listing that understates its size.
fn watched(cmd: Cmd, dest: &Path, limit: u64) -> Cmd {
    let dest = dest.to_path_buf();
    let mut last_check = std::time::Instant::now();
    cmd.file_limit(limit.saturating_add(1)).watch(move || {
        // Walking a large tree on every tick would slow the extraction down.
        if last_check.elapsed() < std::time::Duration::from_millis(250) {
            return None;
        }
        last_check = std::time::Instant::now();
        tree_size(&dest)
            .ok()
            .filter(|size| *size > limit)
            .map(|_| too_large())
    })
}

/// Bytes in the regular files below `root`. Hard links count in full, because
/// anything that reads or repacks them gets a full copy each.
fn tree_size(root: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            let meta = std::fs::symlink_metadata(&path)?;
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

fn is_tar(p: &Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 262];
    std::fs::File::open(p)
        .and_then(|mut f| f.read_exact(&mut b))
        .map(|_| &b[257..262] == b"ustar")
        .unwrap_or(false)
}

/// What to do with symbolic links found after extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Links {
    /// Keep links that resolve inside the extraction directory; remove the rest
    /// (dangling or pointing outside). Used for user-facing extraction/repacking.
    KeepInside,
    /// Remove every link (Nashi itself reads the extracted files: EPUB, RTFD).
    RemoveAll,
}

/// Remove symlinks (per `policy`) and special files (FIFOs, sockets, devices)
/// below `root`, so nothing extracted can make us read outside `root`.
pub fn sanitize_tree(root: &Path, policy: Links) -> std::io::Result<()> {
    let canon_root = root.canonicalize()?;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir)? {
            let p = e?.path();
            let meta = std::fs::symlink_metadata(&p)?;
            let ft = meta.file_type();
            if ft.is_symlink() {
                let inside = policy == Links::KeepInside
                    && p.canonicalize().is_ok_and(|t| t.starts_with(&canon_root));
                if !inside {
                    std::fs::remove_file(&p)?;
                }
            } else if ft.is_dir() {
                stack.push(p);
            } else if !ft.is_file() {
                std::fs::remove_file(&p)?;
            }
        }
    }
    Ok(())
}

/// Extract `archive` (of type `ext`) into the existing directory `dest`, then
/// sanitize links per `links`.
pub fn extract_into(
    ctx: &Ctx,
    archive: &Path,
    ext: &str,
    dest: &Path,
    links: Links,
) -> JobResult<()> {
    let limit = expansion_limit();
    let result = extract_raw(ctx, archive, ext, dest);
    // Past the limit the tools fail on the file they are writing, so check this first.
    if tree_size(dest)? > limit {
        return Err(too_large());
    }
    result?;
    sanitize_tree(dest, links)?;
    if tree_size(dest)? > limit {
        return Err(too_large());
    }
    Ok(())
}

fn extract_raw(ctx: &Ctx, archive: &Path, ext: &str, dest: &Path) -> JobResult<()> {
    let d = ctx.deps;
    let password = ctx.opts.password.as_ref();
    let limit = expansion_limit();
    if ext == "gz" {
        let work = WorkDir::new_in(dest.parent().unwrap_or(dest))?;
        let inner = work.join("inner");
        gunzip(ctx, archive, &inner)?;
        if is_tar(&inner) {
            return extract_raw(ctx, &inner, "tar", dest);
        }
        let name = archive
            .file_name()
            .map(|n| split_name(&n.to_string_lossy()).0)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "file".into());
        std::fs::rename(&inner, dest.join(name))?;
        return Ok(());
    }
    // After the gz step, so the tar inside a .tar.gz is listed as well.
    if listed_size(ctx, archive).is_some_and(|size| size > limit) {
        return Err(too_large());
    }
    if let Some(z) = d.path("7z") {
        // 7z goes by the content, so what was found to be a tar stays one.
        let format: &[&str] = if ext == "tar" { &["-ttar"] } else { &[] };
        let mut cmd = watched(Cmd::new(z), dest, limit)
            .args(["x", "-y", "-bso0", "-bsp0"])
            .args(format)
            .arg_cat("-o", dest)
            .arg(archive);
        if let Some(pw) = password {
            cmd = cmd.secret_stdin(pw);
        }
        let out = cmd.run()?;
        // Exit 2 only because 7z refused unsafe links: that is what we want.
        let only_unsafe_links = out.status.code() == Some(2) && {
            let errs: Vec<&str> = out.stderr.lines().filter(|l| l.contains("ERROR")).collect();
            !errs.is_empty() && errs.iter().all(|l| l.contains("Dangerous link path"))
        };
        if !out.status.success() && !only_unsafe_links {
            return Err(extractor_failed("7z", &out, password.is_some()));
        }
    } else if let Some(b) = d.path("bsdtar") {
        let mut cmd = watched(Cmd::new(b), dest, limit)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(dest);
        if let Some(pw) = password {
            cmd = cmd.secret_stdin(pw);
        }
        let out = cmd.run()?;
        if !out.status.success() {
            return Err(extractor_failed("bsdtar", &out, password.is_some()));
        }
    } else if let (Some(t), "tar") = (d.path("tar"), ext) {
        watched(Cmd::new(t), dest, limit)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .run_ok()?;
    } else if let (Some(u), "zip") = (d.path("unzip"), ext) {
        // unzip takes a password only as an argument (-P), which every user
        // on the machine can read, so a protected zip needs 7z or bsdtar.
        if password.is_some() {
            return Err(JobError::missing("7z", "7zip"));
        }
        let out = watched(Cmd::new(u), dest, limit)
            .args(["-q", "-o"])
            .arg(archive)
            .arg("-d")
            .arg(dest)
            .run()?;
        if !out.status.success() {
            return Err(extractor_failed("unzip", &out, false));
        }
    } else {
        return Err(missing_extractor());
    }
    Ok(())
}

/// Top-level entries of `dir`, safe to pass as arguments.
fn entries(dir: &Path) -> JobResult<Vec<String>> {
    let mut v: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        // `./` for every entry: `-x` is an option and `@x` a list file for
        // bsdtar and 7z.
        .map(|n| format!("./{n}"))
        .collect();
    v.sort();
    if v.is_empty() {
        return Err(JobError::invalid_input("the archive is empty"));
    }
    Ok(v)
}

/// Pack the contents of `src_dir` as `kind` (`zip`, `tar`, `gz` = tar.gz).
fn pack(ctx: &Ctx, src_dir: &Path, kind: &str, out: &Path) -> JobResult<()> {
    let d = ctx.deps;
    let names = entries(src_dir)?;
    if let Some(b) = d.path("bsdtar") {
        let flags: &[&str] = match kind {
            "zip" => &["--format", "zip", "-cf"],
            "tar" => &["--format", "pax", "-cf"],
            _ => &["--format", "pax", "-czf"],
        };
        Cmd::new(b)
            .args(flags)
            .arg(out)
            .arg("-C")
            .arg(src_dir)
            .args(&names)
            .run_ok()?;
    } else if let Some(z) = d.path("7z") {
        let seven = |t: &str, archive: &Path, items: &[String], cwd: &Path| {
            Cmd::new(z)
                // -snl stores links: following them would copy the target once per link.
                .args(["a", "-y", "-bso0", "-bsp0", "-snl", &format!("-t{t}")])
                .arg(archive)
                .args(items)
                .cwd(cwd)
                .run_ok()
        };
        match kind {
            "zip" | "tar" => {
                seven(kind, out, &names, src_dir)?;
            }
            _ => {
                let tmp_dir = WorkDir::new_in(out.parent().unwrap_or(src_dir))?;
                let tar = tmp_dir.join("a.tar");
                seven("tar", &tar, &names, src_dir)?;
                seven("gzip", out, &["a.tar".to_string()], tmp_dir.path())?;
            }
        }
    } else if let (Some(t), true) = (d.path("tar"), kind != "zip") {
        let f = if kind == "tar" { "-cf" } else { "-czf" };
        Cmd::new(t)
            .arg(f)
            .arg(out)
            .arg("-C")
            .arg(src_dir)
            .arg("--")
            .args(&names)
            .run_ok()?;
    } else {
        return Err(JobError::missing("7z", "7zip"));
    }
    Ok(())
}

/// Repack an archive into another archive format.
pub fn convert(ctx: &Ctx, input: &Input, target: &str) -> JobResult<PathBuf> {
    ctx.progress(None, "Extracting");
    let work = WorkDir::new_in(&input.out_dir)?;
    let content = work.join("content");
    std::fs::create_dir_all(&content)?;
    extract_into(ctx, &input.path, input.ext(), &content, Links::KeepInside)?;
    crate::process::check_cancel()?;
    ctx.progress(None, "Packing");
    let out_ext = if target == "gz" { "tar.gz" } else { target };
    let staged = input.stage("", out_ext)?;
    let packed = work.join(&format!("out.{out_ext}"));
    pack(ctx, &content, target, &packed)?;
    staged.take_from(&packed)?;
    Ok(staged.commit()?)
}

/// `extractArchive`: contents into `stem-extracted/`.
pub fn extract_tool(ctx: &Ctx, input: &Input) -> JobResult<PathBuf> {
    ctx.progress(None, "Extracting");
    let staged = input.stage_dir("-extracted")?;
    extract_into(
        ctx,
        &input.path,
        input.ext(),
        staged.path(),
        Links::KeepInside,
    )?;
    if std::fs::read_dir(staged.path())?.next().is_none() {
        return Err(JobError::invalid_input("the archive is empty"));
    }
    Ok(staged.commit()?)
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    #[test]
    fn a_running_extractor_is_stopped_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        // Writes for as long as it is allowed to, as an archive with a false listing would.
        let script =
            r#"i=0; while :; do head -c 50000 /dev/zero > "$0/f$i"; i=$((i+1)); sleep 0.02; done"#;
        let started = std::time::Instant::now();
        let result = watched(Cmd::new(Path::new("/bin/sh")), dir.path(), 200_000)
            .args(["-c", script])
            .arg(dir.path())
            .run();
        let error = result.err().expect("the extractor must be stopped");
        assert_eq!(error.message, too_large().message);
        assert!(
            started.elapsed().as_secs() < 20,
            "took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn an_extractor_inside_the_limit_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let out = watched(Cmd::new(Path::new("/bin/sh")), dir.path(), 200_000)
            .args(["-c", r#"head -c 50000 /dev/zero > "$0/f""#])
            .arg(dir.path())
            .run()
            .unwrap();
        assert!(out.status.success());
    }
}
