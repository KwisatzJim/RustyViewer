#!/usr/bin/env python3
"""Prepare local Homebrew recipes using the collected, pinned sources."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    sources = args.sources.resolve()
    destination = args.destination.resolve()
    if destination.exists():
        raise SystemExit("Choose a new destination directory.")
    entries = json.loads((sources / "inventory.json").read_text())
    destination.mkdir(parents=True)
    formula_dir = destination / "Formula"
    formula_dir.mkdir()
    local_urls = {}
    for entry in entries:
        original = sources / entry["file"]
        if entry.get("revision") and not entry.get("sha256"):
            repo = destination / "git-sources" / entry["package"] / (entry["name"] + ".git")
            repo.parent.mkdir(parents=True, exist_ok=True)
            subprocess.run(["git", "clone", "--quiet", "--branch", "source",
                            str(original) + ".bundle", str(repo)], check=True)
            actual = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
            if actual != entry["revision"]:
                raise RuntimeError(f"Git revision mismatch: {repo}")
            local_urls[entry["url"]] = repo.as_uri()
        else:
            if hashlib.sha256(original.read_bytes()).hexdigest() != entry["sha256"]:
                raise RuntimeError(f"Checksum mismatch: {original}")
            # Keep an archive suffix so Homebrew selects the correct unpacker.
            filename = entry["url"].split("?", 1)[0].rsplit("/", 1)[-1]
            local = destination / "archives" / entry["package"] / filename
            local.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(original, local)
            local_urls[entry["url"]] = local.as_uri()
    patches = sources / "homebrew-patches"
    for entry in json.loads((patches / "manifest.json").read_text()):
        patch = patches / entry["path"]
        if hashlib.sha256(patch.read_bytes()).hexdigest() != entry["sha256"]:
            raise RuntimeError(f"Patch checksum mismatch: {patch}")
    shutil.copytree(patches / "Patches", destination / "Patches")
    names = {p.stem for p in (sources / "recipes").glob("*/*.rb")}
    for recipe in (sources / "recipes").glob("*/*.rb"):
        text = recipe.read_text()
        for patch_name in re.findall(r'file "(Patches/[^"]+)"', text):
            if not (destination / patch_name).is_file():
                raise RuntimeError(f"Missing recipe patch: {patch_name}")
        for url, local in local_urls.items():
            text = text.replace('url "' + url + '"', 'url "' + local + '"')
        # Route the retained library dependencies through this local tap.
        text = re.sub(r'depends_on "([^"]+)"',
                      lambda match: 'depends_on "' + ("rustyviewer/sources/" if match[1] in names else "") + match[1] + '"', text)
        target = formula_dir / recipe.name
        target.write_text(text)
        subprocess.run(["ruby", "-c", str(target)], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["git", "init", "--quiet", str(destination)], check=True)
    subprocess.run(["git", "-C", str(destination), "add", "Formula", "Patches"], check=True)
    subprocess.run(["git", "-C", str(destination), "-c", "user.name=RustyViewer Source Builder",
                    "-c", "user.email=source-builder@localhost", "commit", "--quiet",
                    "-m", "Prepare pinned local source recipes"], check=True)
    print(f"Prepared {len(names)} pinned recipes at {destination}")


if __name__ == "__main__":
    main()
