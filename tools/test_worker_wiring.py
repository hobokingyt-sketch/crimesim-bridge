"""Source-route checks complement, not replace, executed worker/process tests."""
from pathlib import Path
root = Path(__file__).resolve().parents[1]
read = lambda p: (root / p).read_text(encoding="utf-8")
godot = read("src-tauri/src/core/godot.rs")
runtime = read("src-tauri/src/core/runtime.rs")
transaction = read("src-tauri/src/core/transaction.rs")
worker = read("src-tauri/src/core/worker.rs")
for text in (godot, runtime):
    assert "Command::new" not in text and ".output()" not in text
assert "worker::Phase::Version" in runtime
for phase in ("Import", "Harness", "Export", "Smoke"):
    assert f"worker::Phase::{phase}" in godot
assert "outcome.cleanup_confirmed" in worker
assert "worker_guard.json" in worker and "recover_owned_job" in worker
assert transaction.count("worker::ensure_quiescent()") == 3
assert transaction.index("worker::ensure_quiescent()") < transaction.index("ws.recover()")
assert "worker_pending" in read("src-tauri/src/core/status.rs")
assert "spawn_blocking" in read("src-tauri/src/lib.rs")
assert "fixture-processes" not in read("src-tauri/Cargo.toml")
assert "Phase::" not in read("src-tauri/src/core/play.rs")  # Playing is not a validation worker.
print("WORKER ROUTING CHECKS PASSED (static only)")
