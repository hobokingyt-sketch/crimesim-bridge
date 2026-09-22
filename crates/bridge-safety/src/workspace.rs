use crate::{checkpoint, durable, files::{self, copy_tree, file_hash, fingerprint},
    journal::{Journal, Kind, Package, Phase, Receipt, valid_id}, invalid, Fingerprint, Result};
use std::{fs, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Owning the OS lock is required for every mutation, recovery, snapshot or launch request.
/// The lock file is never unlinked. Process death releases the handle, not its pathname.
pub struct Workspace { root: PathBuf, _lock: fs::File }
pub struct Transaction<'a> { ws: &'a Workspace, journal: Journal }
#[derive(Debug, PartialEq, Eq)]
pub enum Recovery { Clean, Restored, Finalized }

impl Workspace {
    pub fn open(root: &Path) -> Result<Self> {
        files::no_links(root)?;
        fs::create_dir_all(root)?;
        let root = root.canonicalize()?;
        for dir in ["state", "workspace", "builds", "applied", "history"] { files::no_links(&root.join(dir))?; }
        fs::create_dir_all(root.join("state"))?;
        let lock_path = root.join("state/workspace.lock"); files::no_links(&lock_path)?;
        let lock = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(lock_path)?;
        lock.try_lock().map_err(|e| invalid(format!("Workspace busy or lock unavailable: {e}")))?;
        Ok(Self { root, _lock: lock })
    }
    pub fn root(&self) -> &Path { &self.root }
    pub fn source(&self) -> PathBuf { self.root.join("workspace/current_project") }
    pub fn build(&self) -> PathBuf { self.root.join("builds/current") }
    pub fn journal_path(&self) -> PathBuf { self.root.join("state/transaction_journal.json") }
    pub fn receipt_path(&self) -> PathBuf { self.root.join("state/current_receipt.json") }
    fn work(&self, id: &str) -> Result<PathBuf> {
        valid_id(id)?;
        let path = self.root.join("state/transactions").join(id); files::no_links(&path)?; Ok(path)
    }
    pub fn journal(&self) -> Result<Option<Journal>> {
        match fs::read(self.journal_path()) {
            Ok(bytes) => {
                let j: Journal = serde_json::from_slice(&bytes).map_err(|e| invalid(format!(
                    "Recovery required: unreadable or legacy journal ({e}). Nothing was discarded.")))?;
                j.validate()?; Ok(Some(j))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    pub fn receipt(&self) -> Result<Option<Receipt>> {
        match fs::read(self.receipt_path()) {
            Ok(bytes) => {
                let r: Receipt = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
                valid_id(&r.transaction_id)?;
                if r.schema != 1 { return Err(invalid("Unsupported current receipt")); }
                Ok(Some(r))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn save(&self, j: &Journal) -> Result<()> { durable::json(&self.journal_path(), j) }
    fn require_clean(&self) -> Result<()> {
        if self.journal()?.is_some() { return Err(invalid("Recovery pending; operation blocked")); }
        Ok(())
    }
    fn current_matches(&self, source: &Fingerprint, build: &Fingerprint) -> Result<()> {
        if fingerprint(&self.source(), true)? != *source || fingerprint(&self.build(), false)? != *build {
            return Err(invalid("Source/build fingerprint mismatch; retained data left intact"));
        }
        Ok(())
    }
    pub fn begin(&self, kind: Kind, project_id: &str, target_revision: u64, input: Option<&Path>) -> Result<Transaction<'_>> {
        self.require_clean()?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| invalid(e.to_string()))?.as_nanos();
        let id = format!("t-{:x}-{:x}-{:x}", stamp, std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
        let work = self.work(&id)?; fs::create_dir_all(&work)?;
        let package = if let Some(p) = input {
            files::no_links(p)?;
            let p = p.canonicalize()?;
            fs::copy(&p, work.join("input.zip"))?;
            fs::OpenOptions::new().write(true).open(work.join("input.zip"))?.sync_all()?;
            Some(Package { original: p.to_string_lossy().into(), sha256: file_hash(&work.join("input.zip"))? })
        } else { None };
        let j = Journal { schema: 2, transaction_id: id, kind, phase: Phase::Preparing,
            project_id: project_id.into(), target_revision,
            before_source: fingerprint(&self.source(), true)?, before_build: fingerprint(&self.build(), false)?,
            after_source: None, after_build: None, before_receipt: self.receipt()?, package };
        self.save(&j)?; checkpoint("journal_started");
        if j.before_source.present { copy_tree(&self.source(), &work.join("before_source"), true)?; }
        checkpoint("source_snapshot");
        if j.before_build.present { copy_tree(&self.build(), &work.join("before_build"), false)?; }
        checkpoint("build_snapshot");
        Ok(Transaction { ws: self, journal: j })
    }
    /// Latest committed transaction's exact before-image, not a reused integer-revision directory.
    pub fn rollback_source(&self) -> Result<Option<PathBuf>> {
        self.require_clean()?;
        let Some(r) = self.receipt()? else { return Ok(None); };
        self.current_matches(&r.source, &r.build)?;
        let work = self.work(&r.transaction_id)?;
        let j: Journal = serde_json::from_slice(&fs::read(work.join("receipt.json"))?).map_err(|e| invalid(e.to_string()))?;
        j.validate()?;
        if j.transaction_id != r.transaction_id || !matches!(j.phase, Phase::Committed) { return Err(invalid("Rollback receipt mismatch")); }
        if !j.before_source.present { return Ok(None); }
        let p = work.join("before_source");
        if fingerprint(&p, true)? != j.before_source { return Err(invalid("Rollback snapshot failed integrity check")); }
        Ok(Some(p))
    }
    pub fn recover(&self) -> Result<Recovery> {
        let result = self.recover_inner();
        match &result {
            Err(e) => {
                // Failure to write a secondary status file must not hide the original recovery error.
                let _ = durable::write(&self.root.join("state/recovery_error.txt"), e.to_string().as_bytes());
            }
            Ok(_) => { files::remove(&self.root.join("state/recovery_error.txt"))?; }
        }
        result
    }
    fn recover_inner(&self) -> Result<Recovery> {
        let Some(mut j) = self.journal()? else { return Ok(Recovery::Clean); };
        if j.phase == Phase::Committed { self.finalize(&j)?; return Ok(Recovery::Finalized); }
        let work = self.work(&j.transaction_id)?;
        if matches!(j.phase, Phase::Preparing | Phase::Recovered) {
            // Before Ready, promotion cannot have started; partial snapshots are not authoritative.
            self.current_matches(&j.before_source, &j.before_build)?;
        } else {
            // Validate BOTH before-images before changing either live directory.
            if fingerprint(&work.join("before_source"), true)? != j.before_source
                || fingerprint(&work.join("before_build"), false)? != j.before_build {
                return Err(invalid("Recovery snapshot missing or corrupt; mutations blocked"));
            }
            for (path, before, after, source) in [
                (self.source(), &j.before_source, j.after_source.as_ref(), true),
                (self.build(), &j.before_build, j.after_build.as_ref(), false)] {
                let actual = fingerprint(&path, source)?;
                if actual.present && actual != *before && Some(&actual) != after {
                    return Err(invalid("Unexpected live contents during recovery; refusing to overwrite"));
                }
            }
            j.phase = Phase::Restoring; self.save(&j)?; checkpoint("recovery_started");
            self.restore(&work, "source", &self.source(), &j.before_source, true)?;
            self.restore(&work, "build", &self.build(), &j.before_build, false)?;
            self.current_matches(&j.before_source, &j.before_build)?;
        }
        self.restore_receipt(&j)?; checkpoint("recovery_receipt");
        j.phase = Phase::Recovered; self.save(&j)?; checkpoint("recovered");
        durable::write(&self.root.join("state/last_recovery.txt"), format!(
            "Restored pre-transaction source/build for {} ({:?}).", j.transaction_id, j.kind).as_bytes())?;
        // Preserve unique before-images and input pack for diagnosis; only discard candidate/scratch.
        self.cleanup_scratch(&work)?;
        files::remove(&self.journal_path())?; checkpoint("recovery_cleared");
        Ok(Recovery::Restored)
    }
    fn restore_receipt(&self, j: &Journal) -> Result<()> {
        if let Some(r) = &j.before_receipt { durable::json(&self.receipt_path(), r) }
        else { files::remove(&self.receipt_path()) }
    }
    fn restore(&self, work: &Path, name: &str, current: &Path, before: &Fingerprint, source: bool) -> Result<()> {
        if fingerprint(current, source)? == *before { return Ok(()); }
        let scratch = work.join(format!("restore_{name}"));
        if before.present { copy_tree(&work.join(format!("before_{name}")), &scratch, source)?; }
        checkpoint(&format!("recovery_{name}_copied"));
        let discarded = work.join(format!("discard_{name}")); files::remove(&discarded)?;
        if current.exists() { durable::rename(current, &discarded)?; }
        checkpoint(&format!("recovery_{name}_displaced"));
        if before.present { durable::rename(&scratch, current)?; }
        checkpoint(&format!("recovery_{name}_restored"));
        Ok(())
    }
    fn cleanup_scratch(&self, work: &Path) -> Result<()> {
        for n in ["source_sync", "candidate_source", "candidate_build", "displaced_source", "displaced_build", "restore_source", "restore_build", "discard_source", "discard_build"] {
            files::remove(&work.join(n))?;
        }
        Ok(())
    }
    fn finalize(&self, j: &Journal) -> Result<()> {
        let source = j.after_source.as_ref().ok_or_else(|| invalid("Missing committed source fingerprint"))?;
        let build = j.after_build.as_ref().ok_or_else(|| invalid("Missing committed build fingerprint"))?;
        self.current_matches(source, build)?;
        let work = self.work(&j.transaction_id)?;
        durable::json(&work.join("receipt.json"), j)?;
        let receipt = Receipt { schema: 1, transaction_id: j.transaction_id.clone(), project_id: j.project_id.clone(),
            revision: j.target_revision, source: source.clone(), build: build.clone() };
        durable::json(&self.receipt_path(), &receipt)?; checkpoint("receipt_written");
        if let Some(pack) = &j.package {
            let original = Path::new(&pack.original);
            let retained = work.join("input.zip");
            if file_hash(&retained)? != pack.sha256 { return Err(invalid("Retained package hash mismatch; cleanup pending")); }
            let archived = self.root.join("applied").join(format!("{}.zip", j.transaction_id));
            if !archived.is_file() { durable::write(&archived, &fs::read(&retained)?)?; }
            if file_hash(&archived)? != pack.sha256 { return Err(invalid("Applied archive hash mismatch; cleanup pending")); }
            // Do not delete a different file later downloaded to the same path.
            if original.exists() && file_hash(original)? == pack.sha256 { files::remove(original)?; }
        }
        checkpoint("package_archived");
        self.cleanup_scratch(&work)?; checkpoint("cleanup_complete");
        durable::write(&self.root.join("state/last_recovery.txt"), format!(
            "Committed transaction {} finalized; source and build revision {} agree.", j.transaction_id, j.target_revision).as_bytes())?;
        files::remove(&self.journal_path())?; checkpoint("journal_cleared");
        Ok(())
    }
}

impl Transaction<'_> {
    pub fn source(&self) -> Result<PathBuf> { Ok(self.ws.work(&self.journal.transaction_id)?.join("candidate_source")) }
    pub fn package(&self) -> Result<Option<PathBuf>> {
        Ok(self.journal.package.as_ref().map(|_| self.ws.work(&self.journal.transaction_id).map(|p| p.join("input.zip"))).transpose()?)
    }
    pub fn promote(mut self, validated_build: &Path, report: &serde_json::Value) -> Result<()> {
        if report.get("passed").and_then(|v| v.as_bool()) != Some(true)
            || report.get("revision").and_then(|v| v.as_u64()) != Some(self.journal.target_revision)
            || report.get("steps").and_then(|v| v.as_array()).map(|s| s.is_empty() || s.iter().any(|v| v["status"] == "failed")).unwrap_or(true)
            || report.get("errors").and_then(|v| v.as_array()).map(|v| !v.is_empty()).unwrap_or(false) {
            return Err(invalid("Required validation did not pass for the target revision"));
        }
        let source = self.source()?;
        let meta: serde_json::Value = serde_json::from_slice(&fs::read(source.join("project_control/bridge_project.json"))?).map_err(|e| invalid(e.to_string()))?;
        if meta["project_id"].as_str() != Some(self.journal.project_id.as_str())
            || meta["revision"].as_u64() != Some(self.journal.target_revision)
            || !source.join("project.godot").is_file() || !validated_build.join("CrimeSim.exe").is_file() {
            return Err(invalid("Validated candidate identity or executable is missing"));
        }
        let work = self.ws.work(&self.journal.transaction_id)?;
        let build = work.join("candidate_build"); copy_tree(validated_build, &build, false)?;
        durable::write(&build.join("bridge_revision.txt"), self.journal.target_revision.to_string().as_bytes())?;
        durable::json(&build.join("bridge_validation.json"), report)?;
        // Source files authored by callers must be synced as well. Copy to a private durable tree.
        let durable_source = work.join("source_sync"); copy_tree(&source, &durable_source, true)?;
        files::remove(&source)?; durable::rename(&durable_source, &source)?;
        self.journal.after_source = Some(fingerprint(&source, true)?);
        self.journal.after_build = Some(fingerprint(&build, false)?);
        if fingerprint(&work.join("before_source"), true)? != self.journal.before_source
            || fingerprint(&work.join("before_build"), false)? != self.journal.before_build {
            return Err(invalid("Before-image verification failed"));
        }
        self.ws.current_matches(&self.journal.before_source, &self.journal.before_build)?;
        self.journal.phase = Phase::Ready; self.ws.save(&self.journal)?; checkpoint("ready");
        self.journal.phase = Phase::SourcePromoting; self.ws.save(&self.journal)?; checkpoint("source_promoting");
        self.swap(&source, &self.ws.source(), &work.join("displaced_source"), "source")?;
        self.journal.phase = Phase::SourcePromoted; self.ws.save(&self.journal)?; checkpoint("source_promoted");
        self.journal.phase = Phase::BuildPromoting; self.ws.save(&self.journal)?; checkpoint("build_promoting");
        self.swap(&build, &self.ws.build(), &work.join("displaced_build"), "build")?;
        self.journal.phase = Phase::PairPromoted; self.ws.save(&self.journal)?; checkpoint("pair_promoted");
        self.ws.current_matches(self.journal.after_source.as_ref().unwrap(), self.journal.after_build.as_ref().unwrap())?;
        self.journal.phase = Phase::Committed; self.ws.save(&self.journal)?; checkpoint("committed");
        self.ws.finalize(&self.journal)
    }
    fn swap(&self, next: &Path, current: &Path, old: &Path, name: &str) -> Result<()> {
        files::no_links(current)?;
        if old.exists() { return Err(invalid("Unexpected displaced tree; recovery required")); }
        fs::create_dir_all(current.parent().ok_or_else(|| invalid("No live parent"))?)?;
        if current.exists() { durable::rename(current, old)?; }
        checkpoint(&format!("{name}_displaced"));
        durable::rename(next, current)?; checkpoint(&format!("{name}_installed"));
        Ok(())
    }
}
