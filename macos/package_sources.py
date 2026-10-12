#!/usr/bin/env python3
"""Create a reviewable source snapshot; release completeness is audited separately."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def main():
    project = Path(__file__).resolve().parent.parent
    sources = project / "target/release/video-sources"
    if not (sources / "inventory.json").is_file():
        raise SystemExit("Collect video sources first.")
    version = json.loads((project / "tauri.conf.json").read_text())["version"]
    output = project / "target/release/bundle/sources"
    output.mkdir(parents=True, exist_ok=True)
    notices = project / "target/release/bundle/macos/RustyViewer.app/Contents/Resources/native-video/ThirdPartyNotices"
    manifest = json.loads((notices / "manifest.json").read_text())
    if manifest.get("version") != version or not manifest.get("source_sha256"):
        raise SystemExit("Rebuild the app to record matching source provenance first.")
    for name, expected in manifest["source_sha256"].items():
        if hashlib.sha256((project / name).read_bytes()).hexdigest() != expected:
            raise SystemExit(f"App source changed since build: {name}; rebuild first.")
    architecture = "aarch64" if manifest["architectures"] == ["arm64"] else "-".join(manifest["architectures"])
    archive = output / f"RustyViewer_{version}_macos_{architecture}_source.tar.gz"
    with tempfile.TemporaryDirectory(prefix="rustyviewer-source-package-") as temporary:
        root = Path(temporary) / f"RustyViewer-{version}-source"
        app = root / "RustyViewer"
        app.mkdir(parents=True)
        paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=project).decode().split("\0")
        for name in sorted(set(paths)):
            if not name:
                continue
            original = project / name
            if not original.is_file():
                continue
            destination = app / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, destination)
        shutil.copytree(sources, root / "video-sources", ignore=shutil.ignore_patterns("*.partial"))
        vendor = project / "target/release/rust-vendor"
        if not vendor.is_dir():
            raise SystemExit("Run cargo vendor --locked target/release/rust-vendor first.")
        shutil.copytree(vendor, app / "vendor")
        (app / ".cargo").mkdir(exist_ok=True)
        (app / ".cargo/config.toml").write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "vendor"\n')
        shutil.copytree(notices, root / "bundle-provenance")
        shutil.copy2(project / "macos/SOURCE_BUILD.md", root / "BUILD.md")
        checksums = []
        for path in sorted(root.rglob("*")):
            if path.is_file():
                checksums.append(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + str(path.relative_to(root)))
        (root / "SHA256SUMS").write_text("\n".join(checksums) + "\n")
        with tarfile.open(archive, "w:gz") as packed:
            packed.add(root, arcname=root.name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + ".sha256").write_text(checksum + "  " + archive.name + "\n")
    print(f"Created source package: {archive}")


if __name__ == "__main__":
    main()
