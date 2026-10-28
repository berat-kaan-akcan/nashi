//! Archives: 7z, falling back to bsdtar / tar / unzip / gzip.

use std::path::{Path, PathBuf};

use super::{Ctx, Input};
use crate::detect::split_name;
use crate::events::{JobError, JobResult};
use crate::fsops::WorkDir;
use crate::process::Cmd;

fn missing_extractor() -> JobError {
    JobError::missing("7z", "7zip")
}

/// Decompress a `.gz` stream into `out`.
fn gunzip(ctx: &Ctx, src: &Path, out: &Path) -> JobResult<()> {
    let d = ctx.deps;
    let f = std::fs::File::create(out)?;
    if let Some(gz) = d.path("gzip") {
        Cmd::new(gz).arg("-dc").arg(src).stdout_to(f).run_ok()?;
    } else if let Some(z) = d.path("7z") {
        Cmd::new(z)
            .args(["e", "-so", "-tgzip"])
            .arg(src)
            .stdout_to(f)
            .run_ok()?;
    } else if let Some(b) = d.path("bsdtar") {
        Cmd::new(b).args(["-xOf"]).arg(src).stdout_to(f).run_ok()?;
    } else {
        return Err(missing_extractor());
    }
    Ok(())
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
    extract_raw(ctx, archive, ext, dest)?;
    sanitize_tree(dest, links)?;
    Ok(())
}

fn extract_raw(ctx: &Ctx, archive: &Path, ext: &str, dest: &Path) -> JobResult<()> {
    let d = ctx.deps;
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
    if let Some(z) = d.path("7z") {
        let out = Cmd::new(z)
            .args(["x", "-y", "-bso0", "-bsp0"])
            .arg_cat("-o", dest)
            .arg(archive)
            .run()?;
        // Exit 2 only because 7z refused unsafe links: that is what we want.
        let only_unsafe_links = out.status.code() == Some(2) && {
            let errs: Vec<&str> = out.stderr.lines().filter(|l| l.contains("ERROR")).collect();
            !errs.is_empty() && errs.iter().all(|l| l.contains("Dangerous link path"))
        };
        if !out.status.success() && !only_unsafe_links {
            return Err(crate::process::failure("7z", &out));
        }
    } else if let Some(b) = d.path("bsdtar") {
        Cmd::new(b)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .run_ok()?;
    } else if let (Some(t), "tar") = (d.path("tar"), ext) {
        Cmd::new(t)
            .arg("-xf")
            .arg(archive)
            .arg("-C")
            .arg(dest)
            .run_ok()?;
    } else if let (Some(u), "zip") = (d.path("unzip"), ext) {
        Cmd::new(u)
            .args(["-q", "-o"])
            .arg(archive)
            .arg("-d")
            .arg(dest)
            .run_ok()?;
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
                .args(["a", "-y", "-bso0", "-bsp0", &format!("-t{t}")])
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
