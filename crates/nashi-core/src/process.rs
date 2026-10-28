//! Running external programs: argv only (never a shell), each child in its own
//! process group so cancellation can kill the whole tree.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::deps::prog_name;
use crate::events::{JobError, JobResult};

fn flag() -> &'static Arc<AtomicBool> {
    static F: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    F.get_or_init(|| Arc::new(AtomicBool::new(false)))
}

/// Install SIGINT/SIGTERM handlers that request cancellation.
pub fn install_signal_handlers() -> std::io::Result<()> {
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(sig, Arc::clone(flag()))?;
    }
    Ok(())
}

pub fn is_cancelled() -> bool {
    flag().load(Ordering::SeqCst)
}

/// Request cancellation programmatically.
pub fn request_cancel() {
    flag().store(true, Ordering::SeqCst);
}

/// Return an error if cancellation was requested.
pub fn check_cancel() -> JobResult<()> {
    if is_cancelled() {
        Err(JobError::cancelled())
    } else {
        Ok(())
    }
}

/// A command line to run.
pub struct Cmd {
    prog: PathBuf,
    args: Vec<OsString>,
    stdout_file: Option<File>,
    envs: Vec<(OsString, OsString)>,
    cwd: Option<PathBuf>,
}

/// Finished process.
pub struct RunOut {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Cmd {
    pub fn new(prog: &Path) -> Cmd {
        Cmd {
            prog: prog.to_path_buf(),
            args: Vec::new(),
            stdout_file: None,
            envs: Vec::new(),
            cwd: None,
        }
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Cmd {
        self.args.push(a.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(mut self, it: I) -> Cmd
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(it.into_iter().map(|s| s.as_ref().to_os_string()));
        self
    }

    /// Argument made of a prefix and a path, e.g. `file:` + path.
    pub fn arg_cat(self, prefix: &str, p: &Path) -> Cmd {
        let mut s = OsString::from(prefix);
        s.push(p.as_os_str());
        self.arg(s)
    }

    pub fn env(mut self, k: &str, v: impl AsRef<OsStr>) -> Cmd {
        self.envs.push((k.into(), v.as_ref().to_os_string()));
        self
    }

    pub fn cwd(mut self, d: &Path) -> Cmd {
        self.cwd = Some(d.to_path_buf());
        self
    }

    /// Send stdout to a file instead of capturing it.
    pub fn stdout_to(mut self, f: File) -> Cmd {
        self.stdout_file = Some(f);
        self
    }

    pub fn name(&self) -> String {
        prog_name(&self.prog)
    }

    /// Run, calling `on_line` for each stdout line. Cancellation kills the
    /// process group and returns `cancelled`. Non-zero exit is *not* an error.
    pub fn run_lines(self, on_line: &mut dyn FnMut(&str)) -> JobResult<RunOut> {
        check_cancel()?;
        let name = self.name();
        let mut c = Command::new(&self.prog);
        c.args(&self.args)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .process_group(0);
        die_with_parent(&mut c);
        // Locale-neutral but UTF-8: avoids tr_TR dotless-i case-folding bugs
        // (ImageMagick fails to find its TIFF coder under tr_TR) and keeps
        // tool output parseable.
        c.env("LC_ALL", "C.UTF-8");
        for (k, v) in &self.envs {
            c.env(k, v);
        }
        if let Some(d) = &self.cwd {
            c.current_dir(d);
        }
        match self.stdout_file {
            Some(f) => {
                c.stdout(Stdio::from(f));
            }
            None => {
                c.stdout(Stdio::piped());
            }
        }
        let mut child = c
            .spawn()
            .map_err(|e| JobError::tool_failed(format!("could not start {name}: {e}")))?;
        let pid = child.id() as i32;

        let mut err_pipe = child.stderr.take().expect("stderr piped");
        let err_thread = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = err_pipe.read_to_end(&mut buf);
            // Keep only the tail; some tools are chatty.
            let start = buf.len().saturating_sub(16 * 1024);
            String::from_utf8_lossy(&buf[start..]).into_owned()
        });
        let (tx, rx) = mpsc::channel::<String>();
        let out_thread = child.stdout.take().map(|out| {
            std::thread::spawn(move || {
                let mut r = BufReader::new(out);
                let mut line = Vec::new();
                loop {
                    line.clear();
                    match r.read_until(b'\n', &mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            if tx
                                .send(String::from_utf8_lossy(&line).into_owned())
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            })
        });

        let mut stdout = String::new();
        let mut handle = |line: String, stdout: &mut String| {
            on_line(line.trim_end_matches(['\n', '\r']));
            if stdout.len() < 8 * 1024 * 1024 {
                stdout.push_str(&line);
            }
        };
        let status = loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(line) => handle(line, &mut stdout),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    std::thread::sleep(Duration::from_millis(30))
                }
            }
            if is_cancelled() {
                kill_group(pid, &mut child);
                return Err(JobError::cancelled());
            }
            match child.try_wait() {
                Ok(Some(st)) => break st,
                Ok(None) => {}
                Err(e) => return Err(JobError::tool_failed(format!("{name}: {e}"))),
            }
        };
        if let Some(t) = out_thread {
            let _ = t.join();
        }
        while let Ok(line) = rx.try_recv() {
            handle(line, &mut stdout);
        }
        let stderr = err_thread.join().unwrap_or_default();
        Ok(RunOut {
            status,
            stdout,
            stderr,
        })
    }

    /// Run and capture; non-zero exit is not an error.
    pub fn run(self) -> JobResult<RunOut> {
        self.run_lines(&mut |_| {})
    }

    /// Run; a non-zero exit becomes `tool_failed` with the stderr tail.
    pub fn run_ok(self) -> JobResult<RunOut> {
        self.run_ok_lines(&mut |_| {})
    }

    pub fn run_ok_lines(self, on_line: &mut dyn FnMut(&str)) -> JobResult<RunOut> {
        let name = self.name();
        let out = self.run_lines(on_line)?;
        if out.status.success() {
            Ok(out)
        } else {
            Err(failure(&name, &out))
        }
    }
}

/// `tool_failed` error describing a finished process.
pub fn failure(name: &str, out: &RunOut) -> JobError {
    let code = out
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "a signal".into());
    let tail: Vec<&str> = out
        .stderr
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let last = tail
        .iter()
        .rev()
        .take(2)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join(" | ");
    if last.is_empty() {
        JobError::tool_failed(format!("{name} exited with {code}"))
    } else {
        JobError::tool_failed(format!("{name} exited with {code}: {last}"))
    }
}

/// How long a cancelled child gets after SIGTERM before SIGKILL. Kept well
/// below the UI's kill timeout (≥ 5 s) so cleanup always finishes.
pub const KILL_GRACE: Duration = Duration::from_millis(1500);

/// Make the child die (SIGKILL) when the spawning thread exits, so a
/// SIGKILLed `nashi-cli` never leaves ffmpeg/soffice/gs running. All jobs spawn
/// from the main thread (probe threads wait for their child), so the
/// "parent thread" caveat of PR_SET_PDEATHSIG does not bite.
pub fn die_with_parent(c: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    // SAFETY: only async-signal-safe calls (prctl, getppid) between fork and exec.
    unsafe {
        c.pre_exec(move || {
            if libc::prctl(
                libc::PR_SET_PDEATHSIG,
                libc::SIGKILL as libc::c_ulong,
                0,
                0,
                0,
            ) == -1
            {
                return Err(std::io::Error::last_os_error());
            }
            // The parent may have died before prctl took effect.
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }
            Ok(())
        });
    }
}

fn kill_group(pid: i32, child: &mut std::process::Child) {
    // SAFETY: plain syscalls on a process group we created.
    unsafe {
        libc::kill(-pid, libc::SIGTERM);
    }
    let deadline = Instant::now() + KILL_GRACE;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            // Leader gone; make sure stragglers in the group go too.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
    let _ = child.wait();
}

/// Tracks ffmpeg `-progress pipe:1` output and converts it to fractions.
pub struct FfProgress {
    duration: Option<f64>,
    last_emit: Instant,
    last_fraction: f64,
}

impl FfProgress {
    pub fn new(duration: Option<f64>) -> FfProgress {
        FfProgress {
            duration: duration.filter(|d| *d > 0.0),
            last_emit: Instant::now() - Duration::from_secs(1),
            last_fraction: -1.0,
        }
    }

    /// Feed one line; returns a fraction when it is worth reporting.
    pub fn feed(&mut self, line: &str) -> Option<f64> {
        let (k, v) = line.split_once('=')?;
        let secs = match k {
            "out_time_us" | "out_time_ms" => v.trim().parse::<f64>().ok()? / 1_000_000.0,
            "progress" if v.trim() == "end" => return Some(1.0),
            _ => return None,
        };
        let d = self.duration?;
        let f = (secs / d).clamp(0.0, 1.0);
        if f - self.last_fraction >= 0.01 && self.last_emit.elapsed() >= Duration::from_millis(200)
        {
            self.last_fraction = f;
            self.last_emit = Instant::now();
            Some((f * 1000.0).round() / 1000.0)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_parse() {
        let mut p = FfProgress::new(Some(10.0));
        assert_eq!(p.feed("out_time_us=5000000"), Some(0.5));
        assert_eq!(p.feed("frame=12"), None);
        assert_eq!(p.feed("progress=end"), Some(1.0));
        let mut q = FfProgress::new(None);
        assert_eq!(q.feed("out_time_us=5000000"), None);
    }

    #[test]
    fn runs_without_shell() {
        let sh = crate::deps::which("echo").unwrap();
        let out = Cmd::new(&sh).arg("a; rm -rf /").run_ok().unwrap();
        assert_eq!(out.stdout.trim(), "a; rm -rf /");
        let f = crate::deps::which("false").unwrap();
        assert!(Cmd::new(&f).run_ok().is_err());
    }
}
