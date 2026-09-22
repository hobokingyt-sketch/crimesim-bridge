CrimeSim Bridge private Godot runtime payload.

Source packages intentionally do not include the large Godot executable/templates.
The Windows CI workflow injects official Godot 4.7.2 files here before building the installer and generates runtime_manifest.json with SHA-256 hashes.
At runtime the Bridge verifies that manifest before allowing Godot validation/export work.
