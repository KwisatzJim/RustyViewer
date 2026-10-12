#!/usr/bin/env python3
"""Package application source and locked Rust dependencies for all platforms."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def main():
    project = Path(__file__).resolve().parent.parent
    version = json.loads((project / "tauri.conf.json").read_text())["version"]
    vendor = project / "target/release/rust-vendor"
    if not vendor.is_dir():
        raise SystemExit("Run cargo vendor --locked target/release/rust-vendor first.")
    output = project / "target/release/bundle/sources"
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"RustyViewer_{version}_application_source.tar.gz"
    with tempfile.TemporaryDirectory(prefix="rustyviewer-app-source-") as temporary:
        root = Path(temporary) / f"RustyViewer-{version}"
        root.mkdir()
        names = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=project).decode().split("\0")
        for name in sorted(set(names)):
            if name and (project / name).is_file():
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(project / name, destination)
        shutil.copytree(vendor, root / "vendor")
        (root / ".cargo").mkdir(exist_ok=True)
        (root / ".cargo/config.toml").write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n\n[source.vendored-sources]\ndirectory = "vendor"\n')
        (root / "SOURCE_BUILD.md").write_text("# Application source build\n\n"
            "This archive includes RustyViewer and its locked Rust dependencies.\n"
            "Install Rust, the Tauri platform prerequisites and the native helper\n"
            "build dependencies described in README.md. Run cargo build --release\n"
            "--locked --offline --features custom-protocol to build without fetching\n"
            "Rust crates. For Linux installers, install libmpv-dev and follow\n"
            "linux/build_app.sh. System libraries and build tools are provided by\n"
            "the platform; npm ci needs network access for development tools.\n"
            "Mac bundled-library sources and recipes are in the separate Mac source\n"
            "archive. See LICENSE and THIRD_PARTY_NOTICES.md for licensing.\n")
        lines = [hashlib.sha256(p.read_bytes()).hexdigest() + "  " + str(p.relative_to(root))
                 for p in sorted(root.rglob("*")) if p.is_file()]
        (root / "SHA256SUMS").write_text("\n".join(lines) + "\n")
        with tarfile.open(archive, "w:gz") as packed:
            packed.add(root, arcname=root.name)
    archive.with_name(archive.name + ".sha256").write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + "  " + archive.name + "\n")
    print(archive)


if __name__ == "__main__":
    main()
