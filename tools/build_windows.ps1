$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
python tools/validate_repo.py
python -m unittest discover -s tools/delivery_tests -v
python tools/test_safety_wiring.py
python tools/test_worker_wiring.py
cargo check --workspace --locked
cargo tauri build --bundles nsis -- --locked
# Installer acceptance/provenance is orchestrated by .github/workflows/build-windows.yml.
