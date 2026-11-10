//! Supervised child processes for agents and checks: a time limit, cancellation, bounded output, and the
//! whole process tree ended on timeout or cancel, so an agent's own children never outlive its attempt
//! (plan §8, scenario 13).

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

#[derive(Clone, Debug)]
pub struct Spec {
    pub program: String,
    pub args: Vec<String>,
    /// Appended after `args` exactly as written, without quoting (Windows `cmd /C` needs the command line
    /// verbatim; Rust's argument quoting would turn its inner quotes into `\"`, which cmd doesn't understand).
    pub raw_tail: Option<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
    /// Bytes kept per stream; output beyond it is dropped from the middle (head and tail are kept).
    pub output_limit: usize,
}

impl Spec {
    /// A command line run through the system shell (`cmd /C` on Windows, `sh -c` elsewhere). Used for checks,
    /// which are command strings the user approved for a workspace.
    pub fn shell(command: &str, cwd: impl Into<PathBuf>, timeout: Duration) -> Spec {
        let (program, args, raw_tail) = if cfg!(windows) {
            // /S: strip the outer quotes we add and run the rest as typed.
            ("cmd".to_string(), vec!["/D".into(), "/S".into(), "/C".into()], Some(format!("\"{command}\"")))
        } else {
            ("sh".to_string(), vec!["-c".into(), command.to_string()], None)
        };
        Spec { program, args, raw_tail, cwd: cwd.into(), env: Vec::new(), timeout, output_limit: 256 * 1024 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Exit {
    Code(i32),
    /// Ended by a signal (Unix) or without an exit code.
    Abnormal,
    TimedOut,
    Canceled,
    /// The program could not be started (not found, no permission).
    FailedToStart(String),
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub exit: Exit,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub duration: Duration,
}

impl Outcome {
    pub fn succeeded(&self) -> bool {
        self.exit == Exit::Code(0)
    }
}

/// Runs `spec` until it exits, its time limit passes, or `cancel` resolves.
pub async fn run(spec: &Spec, cancel: impl std::future::Future<Output = ()>) -> Outcome {
    let started = Instant::now();
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(tail) = &spec.raw_tail {
        append_raw(&mut command, tail);
    }
    isolate_tree(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return Outcome {
                exit: Exit::FailedToStart(e.to_string()),
                stdout: String::new(),
                stderr: String::new(),
                truncated: false,
                duration: started.elapsed(),
            }
        }
    };
    let pid = child.id();
    let limit = spec.output_limit;
    let stdout = tokio::spawn(read_bounded(child.stdout.take(), limit));
    let stderr = tokio::spawn(read_bounded(child.stderr.take(), limit));

    let exit = tokio::select! {
        status = child.wait() => match status {
            Ok(status) => status.code().map(Exit::Code).unwrap_or(Exit::Abnormal),
            Err(_) => Exit::Abnormal,
        },
        _ = tokio::time::sleep(spec.timeout) => Exit::TimedOut,
        _ = cancel => Exit::Canceled,
    };
    if matches!(exit, Exit::TimedOut | Exit::Canceled) {
        if let Some(pid) = pid {
            kill_tree(pid);
        }
        let _ = child.kill().await;
    }

    let (out, out_cut) = stdout.await.unwrap_or_default();
    let (err, err_cut) = stderr.await.unwrap_or_default();
    Outcome { exit, stdout: out, stderr: err, truncated: out_cut || err_cut, duration: started.elapsed() }
}

/// Reads a stream to the end, keeping at most `limit` bytes: the first half and the last half.
async fn read_bounded<R: AsyncRead + Unpin>(stream: Option<R>, limit: usize) -> (String, bool) {
    let Some(mut stream) = stream else { return (String::new(), false) };
    let half = limit / 2;
    let mut head: Vec<u8> = Vec::new();
    let mut tail: std::collections::VecDeque<u8> = std::collections::VecDeque::new();
    let mut dropped = false;
    let mut buf = [0u8; 8192];
    loop {
        let n = match stream.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &byte in &buf[..n] {
            if head.len() < half {
                head.push(byte);
            } else {
                tail.push_back(byte);
                if tail.len() > limit - half {
                    tail.pop_front();
                    dropped = true;
                }
            }
        }
    }
    let mut bytes = head;
    if dropped {
        bytes.extend_from_slice("\n… [output truncated] …\n".as_bytes());
    }
    bytes.extend(tail);
    (String::from_utf8_lossy(&bytes).into_owned(), dropped)
}

#[cfg(windows)]
fn append_raw(command: &mut Command, tail: &str) {
    command.raw_arg(tail);
}

#[cfg(unix)]
fn append_raw(command: &mut Command, tail: &str) {
    command.arg(tail);
}

#[cfg(windows)]
fn isolate_tree(command: &mut Command) {
    // No console window for the child (and its children) in a GUI app.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn isolate_tree(command: &mut Command) {
    // Its own process group, so the whole tree can be signalled at once.
    command.process_group(0);
}

#[cfg(windows)]
fn kill_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(unix)]
fn kill_tree(pid: u32) {
    // SAFETY: sending a signal to a process group id we created; no memory is touched.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
    }

    #[test]
    fn captures_output_and_exit_code() {
        let spec = Spec::shell("echo hello && exit 3", std::env::temp_dir(), Duration::from_secs(20));
        let outcome = rt().block_on(run(&spec, std::future::pending()));
        assert_eq!(outcome.exit, Exit::Code(3));
        assert!(outcome.stdout.contains("hello"));
    }

    #[test]
    fn times_out_and_ends_the_tree() {
        let sleeper = if cfg!(windows) { "ping -n 30 127.0.0.1 > NUL" } else { "sleep 30" };
        let spec = Spec::shell(sleeper, std::env::temp_dir(), Duration::from_millis(500));
        let outcome = rt().block_on(run(&spec, std::future::pending()));
        assert_eq!(outcome.exit, Exit::TimedOut);
        assert!(outcome.duration < Duration::from_secs(15), "took {:?}", outcome.duration);
    }

    #[test]
    fn cancels() {
        let sleeper = if cfg!(windows) { "ping -n 30 127.0.0.1 > NUL" } else { "sleep 30" };
        let spec = Spec::shell(sleeper, std::env::temp_dir(), Duration::from_secs(60));
        // The timer must be created inside the runtime, so the cancel future is built in the async block.
        let outcome = rt().block_on(async { run(&spec, tokio::time::sleep(Duration::from_millis(300))).await });
        assert_eq!(outcome.exit, Exit::Canceled);
    }

    #[test]
    fn reports_a_missing_program() {
        let spec = Spec {
            program: "definitely-not-a-real-program-str".into(),
            args: vec![],
            raw_tail: None,
            cwd: std::env::temp_dir(),
            env: vec![],
            timeout: Duration::from_secs(5),
            output_limit: 1024,
        };
        let outcome = rt().block_on(run(&spec, std::future::pending()));
        assert!(matches!(outcome.exit, Exit::FailedToStart(_)));
    }

    #[test]
    fn bounds_output() {
        let (text, cut) = rt().block_on(read_bounded(Some(&b"abcdefghijklmnopqrstuvwxyz"[..]), 10));
        assert!(cut);
        assert!(text.starts_with("abcde"));
        assert!(text.ends_with("vwxyz"));
    }
}
