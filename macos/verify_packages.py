#!/usr/bin/env python3
"""Verify the local Mac disk image, matching source archive and offline build."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_checksum(path):
    expected = path.with_name(path.name + ".sha256").read_text().split()[0]
    if digest(path) != expected:
        raise RuntimeError(f"Archive checksum mismatch: {path}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline-build", action="store_true")
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    app = project / "target/release/bundle/macos/RustyViewer.app"
    provenance_path = "Contents/Resources/native-video/ThirdPartyNotices/manifest.json"
    manifest = json.loads((app / provenance_path).read_text())
    version = manifest["version"]
    arch = "aarch64" if manifest["architectures"] == ["arm64"] else "-".join(manifest["architectures"])
    dmg = project / f"target/release/bundle/dmg/RustyViewer_{version}_{arch}.dmg"
    source = project / f"target/release/bundle/sources/RustyViewer_{version}_macos_{arch}_source.tar.gz"
    verify_checksum(dmg)
    verify_checksum(source)
    with tempfile.TemporaryDirectory(prefix="rustyviewer-dmg-verify-") as directory:
        mount = Path(directory).resolve()
        subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", str(mount), str(dmg)], check=True, stdout=subprocess.DEVNULL)
        try:
            mounted = mount / "RustyViewer.app"
            if os.readlink(mount / "Applications") != "/Applications":
                raise RuntimeError("Incorrect Applications shortcut")
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(mounted)], check=True)
            original_files = {str(p.relative_to(app)): digest(p) for p in app.rglob("*") if p.is_file()}
            mounted_files = {str(p.relative_to(mounted)): digest(p) for p in mounted.rglob("*") if p.is_file()}
            if original_files != mounted_files:
                raise RuntimeError("Mounted application differs from release build")
            helper = mounted / "Contents/Resources/native-video/RustyViewer Video.app/Contents/MacOS/rustyviewer-video"
            environment = {key: value for key, value in os.environ.items() if not key.startswith("DYLD_")}
            environment["PATH"] = "/usr/bin:/bin"
            result = subprocess.check_output([str(helper), "--check-bundle"], env=environment, text=True)
            loaded = [line.split("\t", 1)[1] for line in result.splitlines() if line.startswith("LIBRARY\t")]
            if not loaded:
                raise RuntimeError("Native-library diagnostic returned no paths")
            external = [p for p in loaded if not p.startswith(("/System/Library/", "/usr/lib/", str(mounted) + "/"))]
            if external:
                raise RuntimeError(f"External native libraries: {external}")
            print(f"Disk image verified: {len(original_files)} matching files; no external native libraries.", flush=True)
        finally:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True, stdout=subprocess.DEVNULL)
    with tarfile.open(source) as packed:
        root = f"RustyViewer-{version}-source"
        lines = packed.extractfile(root + "/SHA256SUMS").read().decode().splitlines()
        checksums = {}
        for line in lines:
            expected, name = line.split("  ", 1)
            actual = hashlib.sha256(packed.extractfile(root + "/" + name).read()).hexdigest()
            if actual != expected:
                raise RuntimeError(f"Source checksum mismatch: {name}")
            checksums[name] = actual
        archived_manifest = json.load(packed.extractfile(root + "/bundle-provenance/manifest.json"))
        if archived_manifest != manifest:
            raise RuntimeError("Source provenance differs from application")
        for name, expected in manifest["source_sha256"].items():
            if checksums.get("RustyViewer/" + name) != expected:
                raise RuntimeError(f"Archived app source differs from build: {name}")
        print(f"Source archive verified: {len(lines)} matching files and matching application provenance.", flush=True)
        if args.offline_build:
            with tempfile.TemporaryDirectory(prefix="rustyviewer-offline-source-") as directory:
                # Extract only this locally generated, checksum-verified archive.
                for member in packed.getmembers():
                    if member.name.startswith("/") or ".." in Path(member.name).parts or member.issym() or member.islnk():
                        raise RuntimeError(f"Unsafe archive member: {member.name}")
                packed.extractall(directory)
                environment = os.environ.copy()
                environment["CARGO_HOME"] = str(Path(directory) / "empty-cargo-home")
                environment["CARGO_TARGET_DIR"] = str(Path(directory) / "build")
                log = project / "target/release/offline-source-build.log"
                with log.open("w") as output:
                    subprocess.run(["cargo", "build", "--release", "--locked", "--offline", "--features", "custom-protocol"],
                                   cwd=Path(directory) / root / "RustyViewer", env=environment,
                                   stdout=output, stderr=subprocess.STDOUT, check=True)
                print(f"Archived application built offline with an empty Cargo cache; log: {log}", flush=True)


if __name__ == "__main__":
    main()
