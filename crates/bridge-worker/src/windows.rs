//! Windows 10+: attach the job atomically at process creation, not after a racy spawn.
use super::{ReadState, Request};
use std::{ffi::{c_void, OsStr}, io, mem::{size_of, size_of_val, zeroed}, os::windows::{ffi::OsStrExt,
    io::{AsRawHandle, FromRawHandle, OwnedHandle}}, ptr::{null, null_mut}, time::{Duration, Instant}};
use windows_sys::Win32::{Foundation::*, Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::*, System::{JobObjects::*, Pipes::*, Threading::*}};

fn wide(s: &OsStr) -> io::Result<Vec<u16>> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    if v.contains(&0) { return Err(io::Error::other("NUL in worker path/argument")); }
    v.push(0); Ok(v)
}
fn handle(raw: HANDLE) -> io::Result<OwnedHandle> {
    if raw.is_null() || raw == INVALID_HANDLE_VALUE { Err(io::Error::last_os_error()) }
    else { Ok(unsafe { OwnedHandle::from_raw_handle(raw) }) }
}
fn raw(h: &OwnedHandle) -> HANDLE { h.as_raw_handle() }
fn bool_ok(value: i32) -> io::Result<()> {
    if value == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}
/// Standard Windows CRT quoting; never invoke cmd.exe or interpolate shell syntax.
fn quoted(arg: &OsStr) -> io::Result<Vec<u16>> {
    let src = wide(arg)?;
    let mut out = vec![34]; let mut slashes = 0;
    for &ch in &src[..src.len() - 1] {
        if ch == 92 { slashes += 1; continue; }
        out.extend(std::iter::repeat_n(92, if ch == 34 { 2 * slashes + 1 } else { slashes }));
        out.push(ch); slashes = 0;
    }
    out.extend(std::iter::repeat_n(92, 2 * slashes)); out.push(34); Ok(out)
}
fn pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = null_mut(); let mut write = null_mut();
    let sa = SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(), bInheritHandle: 1 };
    unsafe {
        bool_ok(CreatePipe(&mut read, &mut write, &sa, 0))?;
        let read = handle(read)?; let write = handle(write)?;
        bool_ok(SetHandleInformation(raw(&read), HANDLE_FLAG_INHERIT, 0))?;
        Ok((read, write))
    }
}
struct Attributes { storage: Vec<usize>, initialized: bool }
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut bytes = 0;
        unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes); }
        if bytes == 0 { return Err(io::Error::last_os_error()); }
        let mut list = Self { storage: vec![0; bytes.div_ceil(size_of::<usize>())], initialized: false };
        unsafe { bool_ok(InitializeProcThreadAttributeList(list.ptr(), 2, 0, &mut bytes))?; }
        list.initialized = true;
        Ok(list)
    }
    fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST { self.storage.as_mut_ptr().cast() }
    fn set(&mut self, attribute: usize, values: &mut [HANDLE]) -> io::Result<()> {
        unsafe { bool_ok(UpdateProcThreadAttribute(self.ptr(), 0, attribute, values.as_mut_ptr().cast(),
            std::mem::size_of_val(values), null_mut(), null_mut())) }
    }
}
impl Drop for Attributes {
    fn drop(&mut self) { if self.initialized { unsafe { DeleteProcThreadAttributeList(self.ptr()); } } }
}
fn active(job: &OwnedHandle) -> io::Result<u32> {
    let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
    unsafe { bool_ok(QueryInformationJobObject(raw(job), JobObjectBasicAccountingInformation,
        (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(), size_of_val(&info) as u32, null_mut()))?; }
    Ok(info.ActiveProcesses)
}

pub(crate) struct Worker {
    job: OwnedHandle,
    process: OwnedHandle,
    pipes: [OwnedHandle; 2],
    disarmed: bool,
}
impl Worker {
    pub(crate) fn spawn(request: &Request) -> io::Result<Self> {
        if request.executable.extension().and_then(|s| s.to_str()).map(|s| s.eq_ignore_ascii_case("exe")) != Some(true) {
            return Err(io::Error::other("Windows worker must be an executable, not a shell script"));
        }
        let executable = wide(request.executable.as_os_str())?;
        let cwd = wide(request.cwd.as_os_str())?;
        let name = wide(OsStr::new(&request.job_name))?;
        let mut command = quoted(request.executable.as_os_str())?;
        for arg in &request.args { command.push(32); command.extend(quoted(arg)?); }
        command.push(0);
        if command.len() > 32767 { return Err(io::Error::other("Worker command line too long")); }
        let job = unsafe { handle(CreateJobObjectW(null(), name.as_ptr()))? };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { return Err(io::Error::other("Worker job identity already exists")); }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe { bool_ok(SetInformationJobObject(raw(&job), JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), size_of_val(&limits) as u32))?; }
        let (stdout, stdout_writer) = pipe()?;
        let (stderr, stderr_writer) = pipe()?;
        let sa = SECURITY_ATTRIBUTES { nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(), bInheritHandle: 1 };
        let nul = wide(OsStr::new("NUL"))?;
        let stdin = unsafe { handle(CreateFileW(nul.as_ptr(), GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE, &sa, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, null_mut()))? };
        let mut jobs = [raw(&job)];
        let mut inherited = [raw(&stdin), raw(&stdout_writer), raw(&stderr_writer)];
        let mut attrs = Attributes::new()?;
        attrs.set(PROC_THREAD_ATTRIBUTE_JOB_LIST as usize, &mut jobs)?;
        attrs.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize, &mut inherited)?;
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = raw(&stdin);
        startup.StartupInfo.hStdOutput = raw(&stdout_writer);
        startup.StartupInfo.hStdError = raw(&stderr_writer);
        startup.lpAttributeList = attrs.ptr();
        let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
        unsafe { bool_ok(CreateProcessW(executable.as_ptr(), command.as_mut_ptr(), null(), null(), 1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
            null::<c_void>(), cwd.as_ptr(), &startup.StartupInfo, &mut info))?; }
        let process = handle(info.hProcess)?;
        let _thread = handle(info.hThread)?;
        Ok(Self { job, process, pipes: [stdout, stderr], disarmed: false })
    }
    pub(crate) fn status(&mut self) -> io::Result<Option<i32>> {
        match unsafe { WaitForSingleObject(raw(&self.process), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                unsafe { bool_ok(GetExitCodeProcess(raw(&self.process), &mut code))?; }
                Ok(Some(code as i32))
            },
            _ => Err(io::Error::last_os_error()),
        }
    }
    pub(crate) fn quiet(&self) -> io::Result<bool> { Ok(active(&self.job)? == 0) }
    pub(crate) fn terminate(&mut self) -> io::Result<()> {
        unsafe { bool_ok(TerminateJobObject(raw(&self.job), 124)) }
    }
    pub(crate) fn disarm(&mut self) { self.disarmed = true; }
    pub(crate) fn read(&mut self, index: usize, buf: &mut [u8]) -> io::Result<ReadState> {
        let mut available = 0;
        let pipe = raw(&self.pipes[index]);
        if unsafe { PeekNamedPipe(pipe, null_mut(), 0, null_mut(), &mut available, null_mut()) } == 0 {
            let error = unsafe { GetLastError() };
            return if error == ERROR_BROKEN_PIPE { Ok(ReadState::Closed) } else { Err(io::Error::from_raw_os_error(error as i32)) };
        }
        if available == 0 { return Ok(ReadState::Pending); }
        let mut got = 0;
        unsafe { bool_ok(ReadFile(pipe, buf.as_mut_ptr(), available.min(buf.len() as u32), &mut got, null_mut()))?; }
        Ok(if got == 0 { ReadState::Closed } else { ReadState::Bytes(got as usize) })
    }
}
impl Drop for Worker {
    fn drop(&mut self) { if !self.disarmed { let _ = self.terminate(); } }
}
pub(crate) fn recover_owned_job(name: &str, timeout: Duration) -> io::Result<()> {
    let name = wide(OsStr::new(name))?;
    let job = unsafe { OpenJobObjectW(JOB_OBJECT_QUERY | JOB_OBJECT_TERMINATE, 0, name.as_ptr()) };
    if job.is_null() {
        let error = unsafe { GetLastError() };
        return if error == ERROR_FILE_NOT_FOUND { Ok(()) } else { Err(io::Error::from_raw_os_error(error as i32)) };
    }
    let job = handle(job)?;
    unsafe { bool_ok(TerminateJobObject(raw(&job), 124))?; }
    let start = Instant::now();
    loop {
        if active(&job)? == 0 { return Ok(()); }
        if start.elapsed() >= timeout { return Err(io::Error::other("Prior worker job did not stop within recovery deadline")); }
        std::thread::sleep(Duration::from_millis(10));
    }
}
