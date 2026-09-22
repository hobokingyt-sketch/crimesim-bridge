"""Static caller-route checks complement, never replace, Rust process-death tests."""
from pathlib import Path
r = Path(__file__).resolve().parents[1]
read = lambda p: (r / p).read_text(encoding="utf-8")
u = read("src-tauri/src/core/update.rs")
p = read("src-tauri/src/core/pipeline.rs")
t = read("src-tauri/src/core/transaction.rs")
assert "transaction::execute(&ws, Kind::Update" in u
assert "transaction::execute(&ws, Kind::Rollback" in u
assert "transaction::execute(&ws, if exists" in p
assert "bootstrap_demo_at(stage)" in p
assert "godot::validate(&stage, meta)" in t and "tx.promote" in t
assert "promote_source(" not in u and "promote_build(" not in u + p
assert ".recover().ok()" not in t + u
assert "fault-injection" not in read("src-tauri/Cargo.toml")
assert "recovery_required" in read("src-tauri/src/core/status.rs")
for f in ["package.rs", "play.rs"]:
    assert "open_recovered()?" in read("src-tauri/src/core/" + f)
print("SAFETY ROUTING CHECKS PASSED (static only)")
