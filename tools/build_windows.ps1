$ErrorActionPreference = "Stop"
Write-Host "CrimeSim Bridge v0.25 preflight"
python tools/validate_repo.py
python tools/test_transaction_model.py
python tools/test_crash_recovery_model.py
python tools/test_runtime_manifest_model.py
python tools/test_pipeline_contract.py
node --check web/app.js
cargo check --workspace
cargo tauri build
