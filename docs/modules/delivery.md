# delivery

## Responsibility
Build configuration, fixtures, installer delivery and installed Windows integration proof.

## Source route
Use the delivery entry in project_control/MODULES.json. Read docs/INSTALLER.md, the Windows
workflow, tools/delivery.py, tools/test_installer.ps1 and src-tauri/src/install_check.rs for BUILD-1.
Keep engine/transaction behavior behind their existing tested interfaces.

## Contract
Installer discovery uses Cargo metadata. A downloadable candidate needs exact source/lock/installer
hashes and a passing native installed-app report. PR runs package and install for acceptance;
artifacts are not equivalent to signed public releases. Existing recovery and worker suites remain.
The installed probe is opt-in, creates a new isolated workspace and never resets existing data.
A real frontend IPC handshake and bundled-resource Godot pipeline must succeed before publication.

## Limits
Fresh hosted-runner app installation is not exhaustive consumer-PC coverage. No code signing,
self-update, missing-WebView download simulation or game save migration is claimed.
Use CURRENT.json and exact-run evidence to distinguish implemented from verified.
