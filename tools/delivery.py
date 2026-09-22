"""Installer discovery/provenance checks. Standard library only; no user setup commands."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import sys


def read_json(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8-sig"))
    if not isinstance(value, dict):
        raise ValueError(f"Expected JSON object: {path}")
    return value


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def discover(metadata: dict) -> Path:
    target = Path(metadata.get("target_directory", ""))
    if not target.is_absolute():
        raise ValueError("cargo metadata target_directory must be absolute")
    folder = target / "release/bundle/nsis"
    candidates = sorted(folder.glob("*-setup.exe"))
    if len(candidates) != 1:
        raise ValueError(f"Expected exactly one NSIS setup executable in {folder}, found {len(candidates)}")
    installer = candidates[0]
    if installer.is_symlink() or not installer.is_file():
        raise ValueError("Installer must be an ordinary file")
    with installer.open("rb") as stream:
        if stream.read(2) != b"MZ":
            raise ValueError("Installer does not have a Windows executable header")
    if installer.stat().st_size < 65536:
        raise ValueError("Installer is suspiciously small")
    return installer.resolve()


def assemble(metadata: dict, report: dict, provenance: dict, lock: Path, output: Path) -> dict:
    installer = discover(metadata)
    packages = [p for p in metadata.get("packages", []) if p.get("name") == "crimesim-bridge"]
    if len(packages) != 1:
        raise ValueError("Metadata must identify one CrimeSim Bridge package")
    commit = provenance.get("source_commit", "")
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("Packaged source commit is missing or invalid")
    for field in ("ok", "frontend_ready", "runtime_verified", "reinstall_preserved_data", "uninstall_preserved_data"):
        if report.get(field) is not True:
            raise ValueError(f"Installed acceptance gate did not pass: {field}")
    for field in ("source_revision", "playable_revision"):
        if type(report.get(field)) is not int or report[field] != 1:
            raise ValueError(f"Unexpected installed {field}")
    version = packages[0]["version"]
    if report.get("bridge_version") != version or report.get("source_commit") != commit:
        raise ValueError("Installed report does not match this source/version")
    required = {"native_frontend_ipc", "bundled_runtime", "initialize", "update", "exported_launch", "rollback", "reapply", "context_pack", "save_canary"}
    if not required.issubset(report.get("checks", [])):
        raise ValueError("Installed acceptance report is incomplete")
    lock_hash = digest(lock)
    if provenance.get("cargo_lock_sha256") != lock_hash:
        raise ValueError("Dependency lock does not match packaged provenance")
    exe_hash = digest(installer)
    if report.get("installer_sha256") != exe_hash:
        raise ValueError("Installed test was not run against this installer")
    if output.exists():
        raise ValueError("Release output must be new; never mix stale artifacts")
    output.mkdir(parents=True)
    shutil.copy2(installer, output / installer.name)
    shutil.copy2(lock, output / "Cargo.lock")
    (output / "install_check.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    manifest = {"schema": 1, "bridge_version": version, "source_commit": commit,
                "source_tree": provenance.get("source_tree"), "installer": installer.name,
                "bytes": installer.stat().st_size, "sha256": exe_hash,
                "cargo_lock_sha256": lock_hash, "godot_version": provenance.get("godot_version"),
                "rust_version": provenance.get("rust_version"), "tauri_cli_version": provenance.get("tauri_cli_version"),
                "installed_check": "passed", "authenticode": "unsigned-development-build"}
    (output / "release_manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    locate = sub.add_parser("discover")
    locate.add_argument("--metadata", type=Path, required=True)
    make = sub.add_parser("manifest")
    for arg in ("metadata", "report", "provenance", "lock", "output"):
        make.add_argument("--" + arg, type=Path, required=True)
    args = parser.parse_args()
    if args.command == "discover":
        print(discover(read_json(args.metadata)))
    else:
        print(json.dumps(assemble(read_json(args.metadata), read_json(args.report),
                                  read_json(args.provenance), args.lock, args.output), indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        print(f"DELIVERY BLOCKED: {error}", file=sys.stderr)
        raise SystemExit(1)
