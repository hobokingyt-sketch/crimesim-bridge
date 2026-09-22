//! Replace in the same directory without unlinking the last valid destination first.
use crate::{checkpoint, Result};
use std::{fs, io::Write, path::Path, sync::atomic::{AtomicU64, Ordering}};
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| crate::invalid("Missing parent"))?;
    fs::create_dir_all(parent)?;
    crate::files::no_links(path)?;
    let tmp = parent.join(format!(".bridge-write-{}-{}-{}.tmp", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| crate::invalid(e.to_string()))?.as_nanos(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let result = (|| {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if path.file_name().and_then(|s| s.to_str()) == Some("transaction_journal.json") {
            checkpoint("journal_temp_synced");
        }
        // On Windows this may fail when a reader denies delete-sharing. Keep the old file intact.
        fs::rename(&tmp, path)?;
        sync_dir(parent)
    })();
    if tmp.exists() { let _ = fs::remove_file(tmp); }
    result
}

pub fn json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    write(path, &serde_json::to_vec_pretty(value).map_err(|e| crate::invalid(e.to_string()))?)
}

pub fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)] { fs::File::open(path)?.sync_all()?; }
    // Directory flush is not portable on Windows. File sync + rename closes the old unlink gap;
    // hard power loss, controller caches and network filesystems are explicitly outside test scope.
    let _ = path;
    Ok(())
}

pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    crate::files::no_links(from)?;
    crate::files::no_links(to)?;
    fs::rename(from, to)?;
    if let Some(p) = from.parent() { sync_dir(p)?; }
    if let Some(p) = to.parent() { sync_dir(p)?; }
    Ok(())
}
