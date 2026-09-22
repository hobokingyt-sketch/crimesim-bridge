//! Tests kill a separate process in production coordinator code. Fixture builds are not Godot tests.
use bridge_safety::{durable, fingerprint, Kind, Phase, Recovery, Workspace};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("bridge-safe-test-{}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&p).unwrap(); Self(p)
    }
}
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn write(p: &Path, s: impl AsRef<[u8]>) { fs::create_dir_all(p.parent().unwrap()).unwrap(); fs::write(p, s).unwrap(); }
fn make_source(root: &Path, rev: u64, label: &str) {
    write(&root.join("project.godot"), "config_version=5\n");
    write(&root.join("project_control/bridge_project.json"), serde_json::to_vec(&serde_json::json!({"project_id":"crime_sim", "revision":rev})).unwrap());
    write(&root.join("game/main.gd"), label);
    write(&root.join("game/main.gd.uid"), "uid://stable-identity");
}
fn make_build(root: &Path, rev: u64, label: &str) {
    write(&root.join("CrimeSim.exe"), label);
    write(&root.join("CrimeSim.pck"), format!("PCK:{label}"));
    write(&root.join("bridge_revision.txt"), rev.to_string());
}
fn setup(root: &Path, mode: &str) {
    write(&root.join("user_saves/keep.sav"), "never replace a save");
    if mode != "initialize" {
        make_source(&root.join("workspace/current_project"), 7, "old-source");
        make_build(&root.join("builds/current"), 7, "old-build");
    }
}
fn target(mode: &str) -> (Kind, u64) {
    match mode { "initialize" => (Kind::Initialize, 0), "rollback" => (Kind::Rollback, 6),
        "repair" => (Kind::Repair, 7), _ => (Kind::Update, 8) }
}
fn report(rev: u64) -> serde_json::Value {
    serde_json::json!({"passed":true,"revision":rev,"steps":[{"name":"fixture-only","status":"passed"}],"errors":[]})
}
fn perform(ws: &Workspace, mode: &str, label: &str) -> std::io::Result<()> {
    let (kind, rev) = target(mode);
    let input = ws.root().join("incoming/CrimeSim_Update_0008.zip");
    if mode == "update" { write(&input, "fixture-package-bytes"); }
    let tx = ws.begin(kind, "crime_sim", rev, (mode == "update").then_some(input.as_path()))?;
    make_source(&tx.source()?, rev, label);
    let candidate = ws.root().join("builds/exported"); make_build(&candidate, rev, label);
    tx.promote(&candidate, &report(rev))
}

#[test]
fn child_entry() {
    let Ok(root) = std::env::var("BRIDGE_CHILD_ROOT") else { return; };
    let root = PathBuf::from(root);
    let mode = std::env::var("BRIDGE_CHILD_MODE").unwrap();
    if mode == "atomic" {
        durable::write(&root.join("state/transaction_journal.json"), b"new-valid-journal").unwrap(); return;
    }
    let ws = Workspace::open(&root).unwrap();
    if mode == "recover" { ws.recover().unwrap(); }
    else if mode == "lock" {
        write(&root.join("signal"), "locked");
        loop { std::thread::sleep(Duration::from_millis(10)); }
    } else { perform(&ws, &mode, "new-source-and-build").unwrap(); }
}

#[cfg(feature = "fault-injection")]
fn kill_at(root: &Path, mode: &str, point: &str) {
    let signal = root.join("signal"); let _ = fs::remove_file(&signal);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_entry", "--nocapture"])
        .env("BRIDGE_CHILD_ROOT", root).env("BRIDGE_CHILD_MODE", mode)
        .env("BRIDGE_TEST_STOP_AT", point).env("BRIDGE_TEST_SIGNAL", &signal)
        .stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !signal.exists() {
        if let Some(status) = child.try_wait().unwrap() { panic!("child ended before {mode}/{point}: {status}"); }
        if Instant::now() >= deadline { child.kill().unwrap(); child.wait().unwrap(); panic!("deadline: {mode}/{point}"); }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Actual OS process termination, not an exception unwinding and cleaning Rust guards.
    child.kill().unwrap(); child.wait().unwrap();
    fs::remove_file(signal).unwrap();
}

#[cfg(feature = "fault-injection")]
const POINTS: &[&str] = &["journal_started", "source_snapshot", "build_snapshot", "ready", "source_promoting",
    "source_displaced", "source_installed", "source_promoted", "build_promoting", "build_displaced",
    "build_installed", "pair_promoted", "committed", "receipt_written", "package_archived", "cleanup_complete", "journal_cleared"];
#[cfg(feature = "fault-injection")]
fn matrix(mode: &str) {
    for (n, point) in POINTS.iter().enumerate() {
        let tmp = Temp::new(); setup(&tmp.0, mode);
        let old_source = fingerprint(&tmp.0.join("workspace/current_project"), true).unwrap();
        let old_build = fingerprint(&tmp.0.join("builds/current"), false).unwrap();
        kill_at(&tmp.0, mode, point);
        let ws = Workspace::open(&tmp.0).unwrap();
        let pending = ws.journal().unwrap();
        let receipt = ws.receipt().unwrap();
        let expected = if n >= 12 {
            if let Some(j) = &pending { (j.after_source.clone().unwrap(), j.after_build.clone().unwrap()) }
            else { let r = receipt.unwrap(); (r.source, r.build) }
        } else { (old_source, old_build) };
        ws.recover().unwrap();
        assert_eq!(fingerprint(&ws.source(), true).unwrap(), expected.0, "{mode}/{point} source");
        assert_eq!(fingerprint(&ws.build(), false).unwrap(), expected.1, "{mode}/{point} build");
        assert_eq!(ws.recover().unwrap(), Recovery::Clean);
        assert_eq!(ws.recover().unwrap(), Recovery::Clean);
        assert_eq!(fs::read_to_string(tmp.0.join("user_saves/keep.sav")).unwrap(), "never replace a save");
    }
}
#[cfg(feature = "fault-injection")]
#[test] fn initialize_process_death_matrix() { matrix("initialize"); }
#[cfg(feature = "fault-injection")]
#[test] fn update_process_death_matrix() { matrix("update"); }
#[cfg(feature = "fault-injection")]
#[test] fn rollback_process_death_matrix() { matrix("rollback"); }
#[cfg(feature = "fault-injection")]
#[test] fn repair_process_death_matrix() { matrix("repair"); }

#[cfg(feature = "fault-injection")]
#[test]
fn recovery_can_itself_be_killed_and_repeated() {
    for mode in ["initialize", "update", "rollback", "repair"] {
        for point in ["recovery_started", "recovery_source_copied", "recovery_source_displaced", "recovery_source_restored",
            "recovery_build_copied", "recovery_build_displaced", "recovery_build_restored", "recovery_receipt", "recovered", "recovery_cleared"] {
            let tmp = Temp::new(); setup(&tmp.0, mode);
            let old = (fingerprint(&tmp.0.join("workspace/current_project"), true).unwrap(), fingerprint(&tmp.0.join("builds/current"), false).unwrap());
            kill_at(&tmp.0, mode, "pair_promoted");
            kill_at(&tmp.0, "recover", point);
            let ws = Workspace::open(&tmp.0).unwrap(); ws.recover().unwrap(); ws.recover().unwrap();
            assert_eq!((fingerprint(&ws.source(),true).unwrap(),fingerprint(&ws.build(),false).unwrap()),old, "{mode}/{point}");
        }
    }
}

#[test]
fn empty_or_legacy_or_corrupt_journal_blocks_begin() {
    for bytes in [b"".as_slice(), b"not json", br#"{"schema":1,"phase":"source_promoted"}"#, br#"{"schema":99}"#] {
        let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
        write(&ws.journal_path(),bytes);
        let before=fingerprint(&ws.source(),true).unwrap();
        assert!(ws.recover().is_err());assert!(ws.begin(Kind::Update,"crime_sim",8,None).is_err());
        assert_eq!(fs::read(ws.journal_path()).unwrap(),bytes);
        assert_eq!(fingerprint(&ws.source(),true).unwrap(),before);
        assert!(tmp.0.join("state/recovery_error.txt").is_file());
    }
}

#[cfg(feature = "fault-injection")]
#[test]
fn missing_or_corrupt_before_image_blocks_both_restores() {
    for name in ["before_source", "before_build"] {
        let tmp=Temp::new();setup(&tmp.0,"update");kill_at(&tmp.0,"update","pair_promoted");
        let ws=Workspace::open(&tmp.0).unwrap();let j=ws.journal().unwrap().unwrap();
        fs::remove_dir_all(tmp.0.join("state/transactions").join(&j.transaction_id).join(name)).unwrap();
        let before=(fingerprint(&ws.source(),true).unwrap(),fingerprint(&ws.build(),false).unwrap());
        assert!(ws.recover().is_err());assert!(ws.recover().is_err());
        assert!(ws.begin(Kind::Rollback,"crime_sim",6,None).is_err());
        assert_eq!((fingerprint(&ws.source(),true).unwrap(),fingerprint(&ws.build(),false).unwrap()),before);
        assert!(ws.journal_path().is_file());
    }
}

#[test]
fn validation_failure_never_promotes() {
    for bad in [serde_json::json!({"passed":false,"revision":8}), report(9), serde_json::json!({"passed":true,"revision":8,"steps":[]})] {
        let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
        let before=(fingerprint(&ws.source(),true).unwrap(),fingerprint(&ws.build(),false).unwrap());
        let tx=ws.begin(Kind::Update,"crime_sim",8,None).unwrap();make_source(&tx.source().unwrap(),8,"new");
        let build=tmp.0.join("export");make_build(&build,8,"new");assert!(tx.promote(&build,&bad).is_err());
        ws.recover().unwrap();
        assert_eq!((fingerprint(&ws.source(),true).unwrap(),fingerprint(&ws.build(),false).unwrap()),before);
    }
}

#[test]
fn missing_executable_and_wrong_source_identity_are_rejected() {
    for missing_exe in [true,false] {
        let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
        let tx=ws.begin(Kind::Update,"crime_sim",8,None).unwrap();
        make_source(&tx.source().unwrap(),if missing_exe {8}else{9},"new");
        let build=tmp.0.join("export");make_build(&build,8,"new");
        if missing_exe {fs::remove_file(build.join("CrimeSim.exe")).unwrap();}
        assert!(tx.promote(&build,&report(8)).is_err());ws.recover().unwrap();
    }
}

#[test]
fn divergent_reapply_uses_new_before_image_not_old_integer_revision() {
    let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
    perform(&ws,"update","branch-A").unwrap();
    let back=ws.rollback_source().unwrap().unwrap();
    let tx=ws.begin(Kind::Rollback,"crime_sim",7,None).unwrap();
    bridge_safety::copy_tree(&back,&tx.source().unwrap(),true).unwrap();
    let build=tmp.0.join("export");make_build(&build,7,"restored");tx.promote(&build,&report(7)).unwrap();
    perform(&ws,"update","branch-B").unwrap();
    let tx=ws.begin(Kind::Update,"crime_sim",9,None).unwrap();make_source(&tx.source().unwrap(),9,"rev9");
    make_build(&build,9,"rev9");tx.promote(&build,&report(9)).unwrap();
    let back=ws.rollback_source().unwrap().unwrap();
    assert_eq!(fs::read_to_string(back.join("game/main.gd")).unwrap(),"branch-B");
}

#[cfg(feature = "fault-injection")]
#[test]
fn committed_archive_failure_is_retryable_not_rolled_back() {
    let tmp=Temp::new();setup(&tmp.0,"update");kill_at(&tmp.0,"update","committed");
    let ws=Workspace::open(&tmp.0).unwrap();write(&tmp.0.join("applied"),"obstruction");
    let expected=fingerprint(&ws.source(),true).unwrap();assert!(ws.recover().is_err());
    assert_eq!(ws.journal().unwrap().unwrap().phase,Phase::Committed);
    assert_eq!(fingerprint(&ws.source(),true).unwrap(),expected);
    fs::remove_file(tmp.0.join("applied")).unwrap();assert_eq!(ws.recover().unwrap(),Recovery::Finalized);
    assert!(!tmp.0.join("incoming/CrimeSim_Update_0008.zip").exists());
}

#[cfg(feature = "fault-injection")]
#[test]
fn replacing_download_after_commit_does_not_delete_new_bytes() {
    let tmp=Temp::new();setup(&tmp.0,"update");kill_at(&tmp.0,"update","committed");
    let input=tmp.0.join("incoming/CrimeSim_Update_0008.zip");write(&input,"different download");
    let ws=Workspace::open(&tmp.0).unwrap();ws.recover().unwrap();
    assert_eq!(fs::read_to_string(input).unwrap(),"different download");
}

#[cfg(feature = "fault-injection")]
#[test]
fn incomplete_journal_replacement_preserves_old_file() {
    let tmp=Temp::new();write(&tmp.0.join("state/transaction_journal.json"),"old-valid-journal");
    kill_at(&tmp.0,"atomic","journal_temp_synced");
    assert_eq!(fs::read_to_string(tmp.0.join("state/transaction_journal.json")).unwrap(),"old-valid-journal");
}

#[test]
fn two_workspace_writers_cannot_overlap() {
    let tmp=Temp::new();let first=Workspace::open(&tmp.0).unwrap();
    assert!(Workspace::open(&tmp.0).is_err());drop(first);assert!(Workspace::open(&tmp.0).is_ok());
}

#[cfg(feature = "fault-injection")]
#[test]
fn process_death_releases_workspace_lock() {
    let tmp=Temp::new();kill_at(&tmp.0,"lock","unused");assert!(Workspace::open(&tmp.0).is_ok());
}

#[cfg(unix)]
#[test]
fn linked_workspace_paths_are_rejected() {
    let tmp=Temp::new();let external=Temp::new();
    std::os::unix::fs::symlink(&external.0,tmp.0.join("workspace")).unwrap();
    assert!(Workspace::open(&tmp.0).is_err());assert_eq!(fs::read_dir(&external.0).unwrap().count(),0);
}

#[test]
fn pending_operation_cannot_be_overwritten() {
    let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
    let _tx=ws.begin(Kind::Update,"crime_sim",8,None).unwrap();let original=fs::read(ws.journal_path()).unwrap();
    assert!(ws.begin(Kind::Initialize,"crime_sim",0,None).is_err());assert_eq!(fs::read(ws.journal_path()).unwrap(),original);
}

#[test]
fn source_uid_is_preserved_and_import_cache_is_not_promoted() {
    let tmp=Temp::new();setup(&tmp.0,"update");let ws=Workspace::open(&tmp.0).unwrap();
    write(&ws.source().join(".godot/cache.tmp"),"discard");perform(&ws,"update","new").unwrap();
    assert_eq!(fs::read_to_string(ws.source().join("game/main.gd.uid")).unwrap(),"uid://stable-identity");
    assert!(!ws.source().join(".godot").exists());
}
