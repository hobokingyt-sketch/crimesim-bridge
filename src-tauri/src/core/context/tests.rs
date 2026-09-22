use super::*;
use serde_json::Value;
use std::io::Read;
use zip::ZipArchive;

struct Fixture { dir: PathBuf, source: PathBuf, out: PathBuf, meta: BridgeProject }
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("bridge-context-test-{}", unique_id()));
        let source = dir.join("source"); let out = dir.join("out");
        fs::create_dir_all(source.join("game")).unwrap();
        fs::create_dir_all(source.join("project_control")).unwrap();
        let meta = crate::core::project::default_metadata();
        let f = Self { dir, source, out, meta };
        f.write("project.godot", b"config_version=5\n");
        f.write("game/main.tscn", b"[gd_scene format=3]\n[node name=\"Main\" type=\"Node\"]\n");
        f.write("game/main.gd", b"extends Node\n");
        f.write(paths::PROJECT_META, &serde_json::to_vec(&f.meta).unwrap());
        seed_new_game(&f.source, &f.meta).unwrap();
        f.write("ui/panel.gd", b"extends Control\n");
        f.write("simulation/clock.gd", b"extends RefCounted\n");
        f.write("assets/portrait.png", b"binary-portrait");
        f.write("assets/portrait.png.import", b"[remap]\n");
        f
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let p = self.source.join(path); fs::create_dir_all(p.parent().unwrap()).unwrap(); fs::write(p,bytes).unwrap();
    }
    fn request(&self, id: &str) -> Request { Request { module_id:id.into(), task:"Improve the selected game area".into(), ..Request::default() } }
    fn export(&self, r: &Request) -> BridgeResult<Pack> { export_at(&self.source,&self.meta,&self.out,r,None) }
    fn fingerprint(&self) -> String { index_hash(&inventory(&self.source,&registry::load(&self.source,&self.meta).unwrap().0).unwrap()).unwrap() }
}
impl Drop for Fixture { fn drop(&mut self) { let _=fs::remove_dir_all(&self.dir); } }
fn read(pack: &Pack, path: &str) -> Vec<u8> {
    let mut z = ZipArchive::new(fs::File::open(&pack.path).unwrap()).unwrap();
    let mut bytes=Vec::new(); z.by_name(path).unwrap().read_to_end(&mut bytes).unwrap(); bytes
}
fn json(pack: &Pack, path: &str) -> Value { serde_json::from_slice(&read(pack,path)).unwrap() }
fn names(pack: &Pack) -> Vec<String> {
    let z=ZipArchive::new(fs::File::open(&pack.path).unwrap()).unwrap(); z.file_names().map(String::from).collect()
}

#[test] fn selected_area_excludes_unrelated_source_and_includes_contracts() {
    let f=Fixture::new();let p=f.export(&f.request("ui")).unwrap();let n=names(&p);
    assert!(n.contains(&"source/ui/panel.gd".into()));
    assert!(!n.contains(&"source/simulation/clock.gd".into()));
    assert!(n.contains(&"source/project_control/modules/foundation.md".into()));
    assert!(p.omitted>0); assert_eq!(json(&p,"context_manifest.json")["project_kind"],"game");
}
#[test] fn exported_bytes_match_every_included_hash() {
    let f=Fixture::new();let p=f.export(&f.request("ui")).unwrap();
    for e in json(&p,"inventory.json").as_array().unwrap() {
        if e["included"]==true { let b=read(&p,&format!("source/{}",e["path"].as_str().unwrap())); assert_eq!(sha256_bytes(&b),e["sha256"]); }
    }
}
#[test] fn export_is_read_only_and_repeatable_at_source_index_level() {
    let f=Fixture::new();let before=f.fingerprint();let a=f.export(&f.request("ui")).unwrap();let b=f.export(&f.request("ui")).unwrap();
    assert_eq!(f.fingerprint(),before);assert_eq!(a.source_index_sha256,b.source_index_sha256);assert_ne!(a.path,b.path);
}
#[test] fn legacy_game_exports_without_mutating_it() {
    let f=Fixture::new();fs::remove_file(f.source.join(REGISTRY_PATH)).unwrap();
    let before=f.fingerprint();let p=f.export(&Request::default()).unwrap();
    assert_eq!(p.memory_origin,"read_only_legacy_routes");assert_eq!(f.fingerprint(),before);
    assert!(!f.source.join(REGISTRY_PATH).exists());assert!(names(&p).contains(&"handoff/LEGACY_README.md".into()));
}
#[test] fn unknown_area_does_not_fall_back_to_entire_project() {
    let f=Fixture::new();assert!(f.export(&f.request("typo")).is_err());assert!(!f.out.exists());
}
#[test] fn binary_asset_is_catalog_only_until_requested() {
    let f=Fixture::new();let p=f.export(&f.request("data")).unwrap();
    assert!(!names(&p).contains(&"source/assets/portrait.png".into()));
    let row=json(&p,"inventory.json");assert!(row.as_array().unwrap().iter().any(|e|e["path"]=="assets/portrait.png" && e["reason"]=="asset_available_on_request"));
}
#[test] fn hash_bound_asset_request_includes_import_companion() {
    let f=Fixture::new();let mut r=f.request("ui");r.file_requests.push(FileRequest{path:"assets/portrait.png".into(),sha256:sha256_bytes(b"binary-portrait")});
    let p=f.export(&r).unwrap();assert_eq!(read(&p,"source/assets/portrait.png"),b"binary-portrait");
    assert!(names(&p).contains(&"source/assets/portrait.png.import".into()));
}
#[test] fn stale_or_missing_requested_file_rejects_whole_export() {
    let f=Fixture::new();let mut r=f.request("ui");
    r.file_requests.push(FileRequest{path:"assets/portrait.png".into(),sha256:"0".repeat(64)});assert!(f.export(&r).is_err());
    r.file_requests[0].path="assets/missing.png".into();assert!(f.export(&r).is_err());assert!(!f.out.exists());
}
#[test] fn large_source_is_omitted_whole_never_truncated() {
    let f=Fixture::new();f.write("ui/large.gd",&vec![b'a';FILE_LIMIT as usize+1]);let p=f.export(&f.request("ui")).unwrap();
    assert!(!names(&p).contains(&"source/ui/large.gd".into()));
    assert!(json(&p,"inventory.json").as_array().unwrap().iter().any(|e|e["path"]=="ui/large.gd"&&e["reason"]=="text_budget_omitted_complete_file"));
}
#[test] fn oversized_required_context_rejects_pack() {
    let f=Fixture::new();f.write("AGENTS.md",&vec![b'a';FILE_LIMIT as usize+1]);assert!(f.export(&f.request("ui")).is_err());
}
#[test] fn saves_caches_and_sensitive_filenames_never_enter_inventory() {
    let f=Fixture::new();for p in ["saves/player.json",".godot/cache","ui/.env","ui/credentials.json"] { f.write(p,b"do-not-export"); }
    let p=f.export(&f.request("ui")).unwrap();
    let entries: Vec<Entry>=serde_json::from_slice(&read(&p,"inventory.json")).unwrap();
    let payload_names=names(&p);
    for forbidden in ["saves/player.json",".godot/cache","ui/.env","ui/credentials.json"] {
        assert!(!entries.iter().any(|e| e.path==forbidden),"Excluded inventory path: {forbidden}");
        assert!(!payload_names.contains(&format!("source/{forbidden}")),"Excluded payload path: {forbidden}");
    }
    assert!(!entries.iter().any(|e|e.path.split('/').any(|part|part==".godot")));
    assert!(entries.iter().any(|e|e.path=="project.godot" && e.included));
}
#[test] fn ordinary_game_secrecy_filename_is_not_a_secret_credential() {
    let f=Fixture::new();f.write("ui/secretive.gd",b"extends Node\n");let p=f.export(&f.request("ui")).unwrap();assert!(names(&p).contains(&"source/ui/secretive.gd".into()));
}
#[test] fn uid_companions_remain_source() {
    let f=Fixture::new();f.write("ui/panel.gd.uid",b"uid://fixture");let p=f.export(&f.request("ui")).unwrap();assert!(names(&p).contains(&"source/ui/panel.gd.uid".into()));
}
#[test] fn invalid_and_duplicate_file_requests_are_rejected() {
    let f=Fixture::new();for path in ["../escape","C:/escape","/absolute","ui\\panel.gd","ui/NUL.txt","ui/bad:stream","ui/../x"] {
        let mut r=f.request("ui");r.file_requests.push(FileRequest{path:path.into(),sha256:"0".repeat(64)});assert!(f.export(&r).is_err(),"{path}");
    }
    let mut r=f.request("ui");let req=FileRequest{path:"assets/portrait.png".into(),sha256:sha256_bytes(b"binary-portrait")};r.file_requests=vec![req.clone(),req];assert!(f.export(&r).is_err());
}
#[test] fn requests_cannot_bypass_privacy_filter() {
    let f=Fixture::new();f.write("ui/.env",b"secret");let mut r=f.request("ui");r.file_requests.push(FileRequest{path:"ui/.env".into(),sha256:sha256_bytes(b"secret")});assert!(f.export(&r).is_err());
}
#[test] fn bridge_registry_cannot_masquerade_as_game_memory() {
    let f=Fixture::new();let mut r=registry::defaults(&f.meta);r.project_kind="bridge".into();f.write(REGISTRY_PATH,&serde_json::to_vec(&r).unwrap());assert!(f.export(&Request::default()).is_err());
}
#[test] fn broken_cards_cycles_and_ambiguous_ownership_are_errors() {
    let f=Fixture::new();let mut r=registry::defaults(&f.meta);r.modules[1].depends_on=vec!["ui".into()];assert!(r.validate(&f.meta.project_id).is_err());
    let mut r=registry::defaults(&f.meta);r.modules[1].paths.push("game/".into());assert!(r.owner("game/main.gd").is_err());
    fs::remove_file(f.source.join("project_control/modules/ui.md")).unwrap();assert!(f.export(&f.request("ui")).is_err());
}
#[test] fn malformed_registry_never_silently_resets_to_defaults() {
    let f=Fixture::new();f.write(REGISTRY_PATH,b"not json");assert!(f.export(&Request::default()).is_err());
}
#[test] fn task_and_explicit_byte_budgets_are_enforced() {
    let f=Fixture::new();let mut r=f.request("ui");r.task="a".repeat(4097);assert!(f.export(&r).is_err());
    let asset=vec![3;REQUEST_BUDGET as usize+1];f.write("assets/large.bin",&asset);let mut r=f.request("ui");r.file_requests.push(FileRequest{path:"assets/large.bin".into(),sha256:sha256_bytes(&asset)});assert!(f.export(&r).is_err());
}
#[test] fn cannot_export_inside_source() {
    let f=Fixture::new();assert!(export_at(&f.source,&f.meta,&f.source.join("out"),&Request::default(),None).is_err());assert!(!f.source.join("out").exists());
}
#[test] fn failed_candidate_report_is_not_presented_as_current_health() {
    let f=Fixture::new();let report=ValidationReport{passed:false,revision:99,created_at:"fixture".into(),steps:vec![],errors:vec!["candidate failed".into()]};
    let p=export_at(&f.source,&f.meta,&f.out,&f.request("ui"),Some(&report)).unwrap();let v=json(&p,"bridge_context/last_validation.json");
    assert_eq!(v["same_source_revision"],false);assert_eq!(v["report"]["passed"],false);
}
#[cfg(unix)] #[test] fn links_and_case_aliases_fail_without_pack_publication() {
    let f=Fixture::new();std::os::unix::fs::symlink("panel.gd",f.source.join("ui/link.gd")).unwrap();assert!(f.export(&f.request("ui")).is_err());fs::remove_file(f.source.join("ui/link.gd")).unwrap();
    f.write("ui/PANEL.gd",b"different");assert!(f.export(&f.request("ui")).is_err());
}
#[test] fn generated_archive_acceptance_checks_actual_payloads() {
    let f=Fixture::new();let p=f.export(&f.request("ui")).unwrap();verify_generated(&p,&f.meta,"ui").unwrap();
    assert!(verify_generated(&p,&f.meta,"simulation").is_err());
}

#[cfg(unix)] #[test] fn unix_literal_backslash_is_not_reinterpreted_as_a_directory() {
    let f=Fixture::new();f.write("ui/bad\\name.gd",b"extends Node\n");assert!(f.export(&f.request("ui")).is_err());
}
#[cfg(unix)] #[test] fn linked_output_parent_does_not_create_directories_inside_source() {
    let f=Fixture::new();let alias=f.dir.join("source-alias");std::os::unix::fs::symlink(&f.source,&alias).unwrap();
    assert!(export_at(&f.source,&f.meta,&alias.join("new-folder"),&f.request("ui"),None).is_err());
    assert!(!f.source.join("new-folder").exists());
}
