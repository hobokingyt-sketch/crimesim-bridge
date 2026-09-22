//! Bounded trusted-worker execution. Not a sandbox or a gameplay lifetime manager.
mod capture;
#[cfg(windows)] mod windows;
#[cfg(windows)] use windows as platform;
#[cfg(target_os = "linux")] mod linux;
#[cfg(target_os = "linux")] use linux as platform;
#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("bridge-worker currently supports Windows and Linux only");
pub use capture::Capture;
use std::{ffi::OsString, io, path::PathBuf, sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};

#[derive(Debug, Clone)]
pub struct Request {
    pub executable: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub timeout: Duration,
    pub cleanup_timeout: Duration,
    pub capture_limit: usize,
    /// A unique, Bridge-owned named job identity. Never a package-supplied command or PID.
    pub job_name: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason { Exited, TimedOut, Cancelled, IoFailure }
#[derive(Debug)]
pub struct Outcome {
    pub reason: Reason,
    pub code: Option<i32>,
    pub cleanup_confirmed: bool,
    pub elapsed: Duration,
    pub stdout: Capture,
    pub stderr: Capture,
    pub detail: String,
}
impl Outcome {
    pub fn success(&self) -> bool {
        self.reason == Reason::Exited && self.code == Some(0) && self.cleanup_confirmed
    }
    pub fn summary(&self) -> String {
        format!("{:?}; exit={:?}; {:.2}s; worker tree stopped={}. {}", self.reason,
            self.code, self.elapsed.as_secs_f64(), self.cleanup_confirmed, self.detail)
    }
    pub fn log(&self) -> String {
        format!("{}\nstdout observed={} omitted={}\nstderr observed={} omitted={}\n--- STDOUT ---\n{}\n--- STDERR ---\n{}",
            self.summary(), self.stdout.observed, self.stdout.omitted_bytes(),
            self.stderr.observed, self.stderr.omitted_bytes(), self.stdout.text(), self.stderr.text())
    }
}
pub(crate) enum ReadState { Bytes(usize), Pending, Closed }
pub fn validate_job_name(name: &str) -> io::Result<()> {
    let suffix = name.strip_prefix("Local\\CrimeSimBridgeWorker_")
        .ok_or_else(|| io::Error::other("Invalid Bridge worker job prefix"))?;
    if suffix.is_empty() || suffix.len() > 100 || !suffix.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
        return Err(io::Error::other("Invalid Bridge worker job identity"));
    }
    Ok(())
}

/// Bounded OS queries and pipe reads. The worker cannot keep a deadline alive by writing logs.
/// Spawn errors occur before a runnable unowned process exists on Windows.
pub fn run(request: &Request, cancel: &AtomicBool) -> io::Result<Outcome> {
    validate_job_name(&request.job_name)?;
    if !request.executable.is_absolute() || !request.cwd.is_absolute()
        || request.timeout.is_zero() || request.timeout > Duration::from_secs(3600)
        || request.cleanup_timeout.is_zero() || request.cleanup_timeout > Duration::from_secs(30)
        || !(1024..=2 * 1024 * 1024).contains(&request.capture_limit) {
        return Err(io::Error::other("Invalid worker paths, deadline or capture budget"));
    }
    let start = Instant::now();
    let mut stdout = Capture::new(request.capture_limit);
    let mut stderr = Capture::new(request.capture_limit);
    if cancel.load(Ordering::Relaxed) {
        return Ok(Outcome { reason: Reason::Cancelled, code: None, cleanup_confirmed: true,
            elapsed: start.elapsed(), stdout, stderr, detail: "Cancelled before spawn".into() });
    }
    let mut child = platform::Worker::spawn(request)?;
    let mut code = None;
    let mut detail = String::new();
    let mut closed = [false; 2];
    let reason = loop {
        let read = drain(&mut child, &mut stdout, &mut stderr, &mut closed);
        if let Err(e) = read { detail = format!("Output read failed: {e}"); break Reason::IoFailure; }
        match child.status() {
            Ok(value) => code = value,
            Err(e) => { detail = format!("Worker status failed: {e}"); break Reason::IoFailure; }
        }
        match child.quiet() {
            Ok(true) if code.is_some() && closed.iter().all(|v| *v) => break Reason::Exited,
            Ok(_) => {},
            Err(e) => { detail = format!("Worker ownership query failed: {e}"); break Reason::IoFailure; }
        }
        if cancel.load(Ordering::Relaxed) { break Reason::Cancelled; }
        if start.elapsed() >= request.timeout { break Reason::TimedOut; }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Stop the owned tree even after the launcher exits. Never wait on an inherited pipe forever.
    if let Err(e) = child.terminate() { detail.push_str(&format!(" Tree termination failed: {e}.")); }
    let cleanup_start = Instant::now();
    let mut cleanup_confirmed = false;
    loop {
        let _ = drain(&mut child, &mut stdout, &mut stderr, &mut closed);
        match child.status() {
            Ok(value) => if code.is_none() { code = value; },
            Err(e) => if !detail.contains("Reap failed:") { detail.push_str(&format!(" Reap failed: {e}.")); },
        }
        if matches!(child.quiet(), Ok(true)) && code.is_some() {
            cleanup_confirmed = true;
            child.disarm();
            break;
        }
        if cleanup_start.elapsed() >= request.cleanup_timeout { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !cleanup_confirmed { detail.push_str(" Cleanup deadline exceeded; preserve staging and block recovery."); }
    Ok(Outcome { reason, code, cleanup_confirmed, elapsed: start.elapsed(), stdout, stderr, detail })
}
fn drain(child: &mut platform::Worker, out: &mut Capture, err: &mut Capture, closed: &mut [bool; 2]) -> io::Result<()> {
    let mut buf = [0; 8192];
    for (index, capture) in [out, err].into_iter().enumerate() {
        if closed[index] { continue; }
        // Fair to both streams and the deadline even with a continuous writer.
        for _ in 0..16 {
            match child.read(index, &mut buf)? {
                ReadState::Bytes(n) => capture.push(&buf[..n]),
                ReadState::Pending => break,
                ReadState::Closed => { closed[index] = true; break; },
            }
        }
    }
    Ok(())
}
/// Windows can reopen the named job after interruption without guessing/reusing a PID.
/// Linux's process groups are for tests/normal timeout handling, not persistent crash recovery.
pub fn recover_owned_job(name: &str, timeout: Duration) -> io::Result<()> {
    validate_job_name(name)?;
    platform::recover_owned_job(name, timeout)
}
