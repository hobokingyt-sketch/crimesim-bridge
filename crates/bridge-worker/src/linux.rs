//! Linux process groups support CI and normal deadlines; not a parent-death sandbox.
use super::{ReadState, Request};
use std::{fs, io::{self, Read}, os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, ChildStderr, ChildStdout, Command, Stdio}, time::Duration};

pub(crate) struct Worker {
    child: Child, stdout: ChildStdout, stderr: ChildStderr, group: i32, disarmed: bool,
}
fn nonblock(fd: i32) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
impl Worker {
    pub(crate) fn spawn(request: &Request) -> io::Result<Self> {
        let mut child = Command::new(&request.executable).args(&request.args).current_dir(&request.cwd)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0).spawn()?;
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let group = child.id() as i32;
        let worker = Self { child, stdout, stderr, group, disarmed: false };
        nonblock(worker.stdout.as_raw_fd())?; nonblock(worker.stderr.as_raw_fd())?;
        Ok(worker)
    }
    pub(crate) fn status(&mut self) -> io::Result<Option<i32>> {
        use std::os::unix::process::ExitStatusExt;
        Ok(self.child.try_wait()?.map(|s| s.code().unwrap_or_else(|| -s.signal().unwrap_or(1))))
    }
    pub(crate) fn quiet(&self) -> io::Result<bool> {
        // A zombie cannot run/write. kill(-pgid, 0) alone incorrectly treats it as a live worker.
        for entry in fs::read_dir("/proc")? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().parse::<u32>().is_err() { continue; }
            let text = match fs::read_to_string(entry.path().join("stat")) {
                Ok(v) => v,
                Err(e) if matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied) => continue,
                Err(e) => return Err(e),
            };
            let Some((_, rest)) = text.rsplit_once(')') else { continue; };
            let values: Vec<_> = rest.split_whitespace().take(3).collect();
            if values.len() == 3 && values[2].parse::<i32>() == Ok(self.group)
                && values[0] != "Z" && values[0] != "X" { return Ok(false); }
        }
        Ok(true)
    }
    pub(crate) fn terminate(&mut self) -> io::Result<()> {
        let result = unsafe { libc::kill(-self.group, libc::SIGKILL) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) { return Err(error); }
        }
        Ok(())
    }
    pub(crate) fn disarm(&mut self) { self.disarmed = true; }
    pub(crate) fn read(&mut self, index: usize, buf: &mut [u8]) -> io::Result<ReadState> {
        let result = if index == 0 { self.stdout.read(buf) } else { self.stderr.read(buf) };
        match result {
            Ok(0) => Ok(ReadState::Closed), Ok(n) => Ok(ReadState::Bytes(n)),
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) => Ok(ReadState::Pending),
            Err(e) => Err(e),
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        if !self.disarmed { let _ = self.terminate(); let _ = self.child.try_wait(); }
    }
}
pub(crate) fn recover_owned_job(_name: &str, _timeout: Duration) -> io::Result<()> {
    Err(io::Error::other("Linux cannot safely recover a persisted Windows job identity; retain workspace for inspection"))
}
