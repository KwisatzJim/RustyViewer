#!/usr/bin/env python3
"""Collect recorded upstream sources; this is one part of release preparation."""
import argparse
import hashlib
import json
import re
import shutil
import tempfile
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    project = Path(__file__).resolve().parent.parent
    notices = project / "target/release/bundle/macos/RustyViewer.app/Contents/Resources/native-video/ThirdPartyNotices"
    manifest = json.loads((notices / "manifest.json").read_text())
    output = project / "target/release/video-sources"
    output.mkdir(parents=True, exist_ok=True)
    entries = []
    cellar = Path(subprocess.check_output(["brew", "--cellar"], text=True).strip())
    for package, metadata in manifest["packages"].items():
        receipt = json.loads((notices / package / "INSTALL_RECEIPT.json").read_text())
        root = cellar / metadata["formula"] / metadata["version"]
        recipe = root / ".brew" / (metadata["formula"] + ".rb")
        if not recipe.is_file():
            raise RuntimeError(f"Missing installed build recipe: {recipe}")
        recipe_dir = output / "recipes" / package
        recipe_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(recipe, recipe_dir / recipe.name)
        shutil.copy2(notices / package / "INSTALL_RECEIPT.json", recipe_dir)
        count = 0
        for source in metadata["sources"]:
            identifier = source["SPDXID"]
            if not identifier.startswith(("SPDXRef-Archive-", "SPDXRef-Patch-", "SPDXRef-Resource-")):
                continue
            checksums = [item["checksumValue"] for item in source.get("checksums", [])
                         if item["algorithm"] == "SHA256"]
            url = source.get("downloadLocation", "")
            if not url.startswith("https://"):
                raise RuntimeError(f"Missing HTTPS source: {package} {identifier}")
            count += 1
            entries.append({"package": package, "name": source["name"],
                            "url": url, "sha256": checksums[0] if len(checksums) == 1 else "",
                            "provenance": source,
                            "revision": receipt.get("source", {}).get("scm_revision"),
                            "file": f"{package}/{identifier}"})
        if not count:
            raise RuntimeError(f"No source archive recorded for {package}")
        # These build resources are absent from the installed SPDX records.
        # Read their pins from the retained recipe, never from current upstream HEAD.
        required = {
            "shaderc": ("glslang", "spirv-headers", "spirv-tools"),
            "libplacebo": ("glad2", "jinja2", "markupsafe"),
            "glib": ("gobject-introspection",),
        }.get(metadata["formula"], ())
        recipe_text = recipe.read_text()
        for name in required:
            match = re.search(r'resource "' + re.escape(name) + r'" do(.*?)(?=\n\s*(?:livecheck|end))', recipe_text, re.S)
            if not match:
                raise RuntimeError(f"Missing resource recipe: {package}/{name}")
            block = match.group(1)
            url_match = re.search(r'url "([^"]+)"', block)
            hash_match = re.search(r'sha256 "([0-9a-f]{64})"', block)
            revision_match = re.search(r'revision: "([0-9a-f]{40})"', block)
            if not url_match or not (hash_match or revision_match):
                raise RuntimeError(f"Unpinned resource: {package}/{name}")
            entries.append({"package": package, "name": name, "url": url_match.group(1),
                            "sha256": hash_match.group(1) if hash_match else "",
                            "revision": revision_match.group(1) if revision_match else None,
                            "provenance": {"recipe": str(recipe.relative_to(root))},
                            "file": f"{package}/resource-{name}"})
    (output / "inventory.json").write_text(json.dumps(entries, indent=2) + "\n")
    print(f"Recorded {len(entries)} source archives and patches for {len(manifest['packages'])} packages.", flush=True)
    if not args.download:
        return
    patches = json.loads((project / "macos/source_patches.json").read_text())
    patch_root = output / "homebrew-patches"
    patch_root.mkdir(exist_ok=True)
    for patch in patches:
        path = patch_root / patch["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        if not path.exists():
            url = f"https://raw.githubusercontent.com/Homebrew/homebrew-core/{patch['revision']}/{patch['path']}"
            subprocess.run(["curl", "--fail", "--location", "--silent", "--show-error",
                            "--proto", "=https", "--proto-redir", "=https",
                            "--output", str(path), url], check=True)
        if hashlib.sha256(path.read_bytes()).hexdigest() != patch["sha256"]:
            raise RuntimeError(f"Patch checksum mismatch: {path}")
    (patch_root / "manifest.json").write_text(json.dumps(patches, indent=2) + "\n")
    unresolved = []
    for index, entry in enumerate(entries, 1):
        if not re.fullmatch(r"[0-9a-fA-F]{64}", entry["sha256"]):
            revision = entry.get("revision")
            if entry["url"].endswith(".git") and re.fullmatch(r"[0-9a-f]{40}", revision or ""):
                bundle = output / (entry["file"] + ".bundle")
                bundle.parent.mkdir(exist_ok=True)
                if not bundle.exists():
                    with tempfile.TemporaryDirectory(prefix="rustyviewer-source-") as temporary:
                        repo = Path(temporary)
                        subprocess.run(["git", "init", "--bare", str(repo)], check=True, stdout=subprocess.DEVNULL)
                        subprocess.run(["git", "-C", str(repo), "fetch", entry["url"], revision], check=True)
                        actual = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "FETCH_HEAD"], text=True).strip()
                        if actual != revision:
                            raise RuntimeError("Source revision mismatch")
                        subprocess.run(["git", "-C", str(repo), "update-ref", "refs/heads/source", revision], check=True)
                        subprocess.run(["git", "-C", str(repo), "bundle", "create", str(bundle), "refs/heads/source"], check=True)
                heads = subprocess.check_output(["git", "bundle", "list-heads", str(bundle)], text=True)
                if heads.strip() != revision + " refs/heads/source":
                    raise RuntimeError(f"Bundle revision mismatch: {bundle}")
                print(f"Verified Git revision: {revision}", flush=True)
                continue
            print(f"Needs revision-based collection: {entry['file']}", flush=True)
            unresolved.append(entry["file"])
            continue
        path = output / entry["file"]
        path.parent.mkdir(exist_ok=True)
        if not path.exists():
            temporary = path.with_name(path.name + ".partial")
            subprocess.run(["curl", "--fail", "--location", "--retry", "2",
                            "--connect-timeout", "30", "--max-time", "600",
                            "--proto", "=https", "--proto-redir", "=https",
                            "--silent", "--show-error", "--output", str(temporary),
                            entry["url"]], check=True)
            if hashlib.sha256(temporary.read_bytes()).hexdigest() != entry["sha256"]:
                temporary.unlink()
                raise RuntimeError(f"Checksum mismatch: {entry['file']}")
            temporary.replace(path)
        if hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
            raise RuntimeError(f"Checksum mismatch: {entry['file']}")
        print(f"Verified {index}/{len(entries)}: {entry['file']}", flush=True)
    if unresolved:
        raise SystemExit(f"Collection incomplete: {len(unresolved)} revision-based sources remain.")


if __name__ == "__main__":
    main()
