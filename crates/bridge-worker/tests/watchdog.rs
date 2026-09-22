use bridge_worker::{run, Reason, Request};
use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicBool, AtomicU64, Ordering}, time::{Duration, Instant, SystemTime, UNIX_EPOCH}};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("worker test é-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&root).unwrap(); Self(root)
    }
    fn request(&self, mode: &str) -> Request {
        Request { executable: PathBuf::from(env!("CARGO_BIN_EXE_worker_fixture")),
            args: vec![mode.into()], cwd: self.0.clone(), timeout: Duration::from_secs(2),
            cleanup_timeout: Duration::from_secs(3), capture_limit: 4096,
            job_name: format!("Local\\CrimeSimBridgeWorker_{}_{}_{}", std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)) }
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn execute(r: &Request) -> bridge_worker::Outcome { run(r, &AtomicBool::new(false)).unwrap() }
fn alive(pid: u32) -> bool {
    #[cfg(windows)] {
        use windows_sys::Win32::{Foundation::CloseHandle, System::Threading::{OpenProcess, WaitForSingleObject}};
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
        if handle.is_null() { return false; }
        let live = unsafe { WaitForSingleObject(handle, 0) == 258 }; unsafe { CloseHandle(handle); } live
    }
    #[cfg(target_os = "linux")] {
        fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|t| t.rsplit_once(')').map(|(_, s)| s.trim_start().starts_with(['Z', 'X']))) == Some(false)
    }
}
fn assert_stopped(root: &Path, minimum: usize) {
    let ids: Vec<u32> = fs::read_dir(root).unwrap().filter_map(Result::ok)
        .filter(|p| p.path().extension().and_then(|s| s.to_str()) == Some("pid"))
        .map(|p| fs::read_to_string(p.path()).unwrap().parse().unwrap()).collect();
    assert!(ids.len() >= minimum, "Fixture descendants never started: {ids:?}");
    let deadline = Instant::now() + Duration::from_secs(3);
    while ids.iter().any(|p| alive(*p)) && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
    assert!(ids.iter().all(|p| !alive(*p)), "Surviving workers: {ids:?}");
}
#[test] fn success_captures_both_streams() {
    let t = Temp::new(); let o = execute(&t.request("success"));
    assert!(o.success(), "{}", o.log()); assert_eq!(o.stdout.text(), "ready\n"); assert!(o.stderr.text().contains("diagnostic"));
}
#[test] fn failure_exit_code_is_not_success() {
    let t = Temp::new(); let o = execute(&t.request("fail"));
    assert_eq!(o.code, Some(7)); assert!(!o.success()); assert!(o.cleanup_confirmed);
}
#[test] fn stdin_is_closed_not_a_hidden_prompt() {
    let t = Temp::new(); let o = execute(&t.request("stdin")); assert!(o.success()); assert_eq!(o.stdout.text(), "0\n");
}
#[test] fn silent_hang_has_real_deadline() {
    let t = Temp::new(); let o = execute(&t.request("sleep"));
    assert_eq!(o.reason, Reason::TimedOut); assert!(o.cleanup_confirmed); assert!(o.elapsed < Duration::from_secs(7));
}
#[test] fn continuous_stdout_and_stderr_cannot_extend_deadline() {
    let t = Temp::new(); let o = execute(&t.request("flood"));
    assert_eq!(o.reason, Reason::TimedOut); assert!(o.cleanup_confirmed);
    for c in [&o.stdout, &o.stderr] { assert!(c.observed > 4096); assert!(c.retained_bytes() <= 4096); assert!(c.omitted_bytes() > 0); }
    assert!(o.log().len() < 12000); assert!(o.elapsed < Duration::from_secs(7));
}
#[test] fn large_output_keeps_ends_and_middle_error_flag() {
    let t = Temp::new(); let mut r = t.request("finite-flood"); r.timeout = Duration::from_secs(10);
    let o = execute(&r); assert!(o.success(), "{}", o.summary());
    assert!(o.stdout.text().starts_with("START")); assert!(o.stdout.text().ends_with("END\n"));
    assert!(o.stdout.error_marker_seen); assert!(o.stdout.omitted_bytes() > 1_000_000);
}
#[test] fn full_tree_is_terminated() {
    let t = Temp::new(); let mut r = t.request("tree"); r.args.push(t.0.clone().into_os_string());
    let o = execute(&r); assert_eq!(o.reason, Reason::TimedOut); assert!(o.cleanup_confirmed); assert_stopped(&t.0, 3);
}
#[test] fn exited_launcher_with_inherited_pipes_is_not_done() {
    let t = Temp::new(); let mut r = t.request("exit-parent"); r.args.push(t.0.clone().into_os_string());
    let o = execute(&r); assert_eq!(o.code, Some(0)); assert_eq!(o.reason, Reason::TimedOut);
    assert!(!o.success()); assert!(o.cleanup_confirmed); assert_stopped(&t.0, 3);
}
#[test] fn descendants_without_pipe_handles_are_still_owned() {
    let t = Temp::new(); let mut r = t.request("closed-pipe-child"); r.args.push(t.0.clone().into_os_string());
    let o = execute(&r); assert_eq!(o.reason, Reason::TimedOut); assert!(o.cleanup_confirmed); assert_stopped(&t.0, 3);
}
#[test] fn cancellation_terminates_tree() {
    let t = Temp::new(); let mut r = t.request("tree"); r.args.push(t.0.clone().into_os_string());
    let token = AtomicBool::new(false);
    let o = std::thread::scope(|s| {
        s.spawn(|| { std::thread::sleep(Duration::from_millis(800)); token.store(true, Ordering::Relaxed); });
        run(&r, &token).unwrap()
    });
    assert_eq!(o.reason, Reason::Cancelled); assert!(o.cleanup_confirmed); assert_stopped(&t.0, 3);
}
#[test] fn cancelled_before_spawn_does_not_start() {
    let t = Temp::new(); let o = run(&t.request("success"), &AtomicBool::new(true)).unwrap();
    assert_eq!(o.reason, Reason::Cancelled); assert_eq!(o.code, None); assert_eq!(o.stdout.observed, 0);
}
#[test] fn missing_executable_and_invalid_limits_fail() {
    let t = Temp::new(); let mut r = t.request("success"); r.executable = t.0.join("not-present.exe");
    assert!(run(&r, &AtomicBool::new(false)).is_err());
    r = t.request("success"); r.timeout = Duration::ZERO; assert!(run(&r, &AtomicBool::new(false)).is_err());
    r = t.request("success"); r.job_name = "external-job".into(); assert!(run(&r, &AtomicBool::new(false)).is_err());
}
#[test] fn path_and_arguments_preserve_spaces_unicode_quotes() {
    let t = Temp::new(); let mut r = t.request("args");
    let args = ["", "two words", "résumé", "trailing\\", "quoted\"value", "\\\"", "literal & %PATH% ^ |"];
    r.args.extend(args.iter().map(|v| std::ffi::OsString::from(*v)));
    let o = execute(&r); assert!(o.success(), "{}", o.log()); assert_eq!(o.stdout.text(), format!("{}\n", args.join("\n")));
}
#[test] fn timeout_then_success_releases_all_resources() {
    let t = Temp::new(); let o = execute(&t.request("sleep")); assert_eq!(o.reason, Reason::TimedOut);
    for _ in 0..3 { assert!(execute(&t.request("success")).success()); }
}
#[test] fn timeout_keeps_source_build_and_saves_unchanged() {
    let t = Temp::new(); let ws = bridge_safety::Workspace::open(&t.0).unwrap();
    fs::create_dir_all(ws.source()).unwrap(); fs::create_dir_all(ws.build()).unwrap();
    fs::write(ws.source().join("kept.gd"), "committed source").unwrap();
    fs::write(ws.build().join("CrimeSim.exe"), "committed build").unwrap();
    fs::write(t.0.join("user.sav"), "save data").unwrap();
    let before = (bridge_safety::fingerprint(&ws.source(), true).unwrap(), bridge_safety::fingerprint(&ws.build(), false).unwrap());
    let tx = ws.begin(bridge_safety::Kind::Repair, "crime_sim", 1, None).unwrap();
    fs::create_dir_all(tx.source().unwrap()).unwrap();
    fs::write(tx.source().unwrap().join("new.gd"), "candidate only").unwrap();
    assert!(bridge_safety::Workspace::open(&t.0).is_err());
    let o = execute(&t.request("sleep")); assert_eq!(o.reason, Reason::TimedOut); assert!(o.cleanup_confirmed);
    ws.recover().unwrap();
    assert_eq!(before, (bridge_safety::fingerprint(&ws.source(), true).unwrap(), bridge_safety::fingerprint(&ws.build(), false).unwrap()));
    assert_eq!(fs::read_to_string(t.0.join("user.sav")).unwrap(), "save data");
    drop(ws); assert!(bridge_safety::Workspace::open(&t.0).is_ok());
}
#[cfg(windows)]
#[test] fn killing_supervisor_kills_windows_job_then_recovery_is_idempotent() {
    use std::process::{Command, Stdio};
    let t = Temp::new(); let request = t.request("tree");
    let mut supervisor = Command::new(&request.executable).arg("supervisor").arg(&t.0).arg(&request.job_name)
        .stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !t.0.join("grandchild.pid").exists() && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
    let started = t.0.join("grandchild.pid").exists();
    supervisor.kill().unwrap(); supervisor.wait().unwrap();
    // No Rust destructor ran in the supervisor: Windows closed its last non-inherited job handle.
    bridge_worker::recover_owned_job(&request.job_name, Duration::from_secs(3)).unwrap();
    bridge_worker::recover_owned_job(&request.job_name, Duration::from_secs(3)).unwrap();
    assert!(started); assert_stopped(&t.0, 3);
}
