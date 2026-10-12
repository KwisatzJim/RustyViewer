#!/usr/bin/env python3
"""Copy the native player and its Mach-O dependencies into a relocatable app.

No Homebrew files are modified. Only the newly built app's copies are rewritten.
"""
import hashlib
import json
import plistlib
import re
import shutil
import subprocess
import sys
from pathlib import Path


def run(*args):
    result = subprocess.run(args, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f"{args[0]} failed: {result.stderr or result.stdout}")
    return result.stdout.strip()


def dependencies(path):
    return [line.strip().split(" (compatibility", 1)[0]
            for line in run("/usr/bin/otool", "-L", str(path)).splitlines()[1:]]


def system_library(name):
    return name.startswith(("/System/Library/", "/usr/lib/"))


def rpaths(path):
    return re.findall(r"cmd LC_RPATH\s+cmdsize \d+\s+path (.*?) \(offset", run("/usr/bin/otool", "-l", str(path)))


def resolve(name, owner, executable):
    if name.startswith("@loader_path/"):
        return (owner.parent / name.removeprefix("@loader_path/")).resolve(strict=True)
    if name.startswith("@executable_path/"):
        return (executable.parent / name.removeprefix("@executable_path/")).resolve(strict=True)
    if name.startswith("@rpath/"):
        for root in rpaths(owner) + rpaths(executable):
            root = root.replace("@loader_path", str(owner.parent)).replace("@executable_path", str(executable.parent))
            candidate = Path(root) / name.removeprefix("@rpath/")
            if candidate.is_file():
                return candidate.resolve()
        raise RuntimeError(f"Cannot resolve {name} from {owner}")
    return Path(name).resolve(strict=True)


def minimum_os(path):
    text = run("/usr/bin/otool", "-l", str(path))
    versions = re.findall(r"\bminos (\d+(?:\.\d+)+)", text)
    versions += re.findall(r"cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+(?:\.\d+)+)", text)
    return max((tuple(map(int, value.split("."))) for value in versions), default=(11, 0))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_root(path):
    for root in path.parents:
        if (root / "INSTALL_RECEIPT.json").is_file():
            return root
    raise RuntimeError(f"Missing provenance for {path}")


def bundle(app):
    project = Path(__file__).resolve().parents[1]
    source = project / "target/native-video/RustyViewer Video.app"
    resources = app / "Contents/Resources"
    destination = resources / "native-video/RustyViewer Video.app"
    if destination.exists():
        shutil.rmtree(destination)
    shutil.copytree(source, destination)
    binary = destination / "Contents/MacOS/rustyviewer-video"
    frameworks = destination / "Contents/Frameworks"
    frameworks.mkdir()
    original = source / "Contents/MacOS/rustyviewer-video"
    sources = {original.resolve(): binary}
    names = {}
    links = {}
    queue = [original.resolve()]
    minimum = minimum_os(binary)
    arch = run("/usr/bin/lipo", "-archs", str(binary)).split()
    while queue:
        owner = queue.pop()
        links[owner] = {}
        for name in dependencies(owner):
            if system_library(name):
                continue
            dependency = resolve(name, owner, original)
            if dependency == owner:  # dylib install ID, not a dependency
                continue
            links[owner][name] = dependency
            if dependency in sources:
                continue
            basename = dependency.name
            if basename in names and names[basename] != dependency:
                raise RuntimeError(f"Conflicting library basename: {basename}")
            names[basename] = dependency
            copied = frameworks / basename
            shutil.copy2(dependency, copied)
            copied.chmod(copied.stat().st_mode | 0o200)
            sources[dependency] = copied
            minimum = max(minimum, minimum_os(dependency))
            if not set(arch).issubset(run("/usr/bin/lipo", "-archs", str(dependency)).split()):
                raise RuntimeError(f"Architecture mismatch: {dependency}")
            queue.append(dependency)

    for owner, copied in sources.items():
        edits = []
        if copied != binary:
            edits += ["-id", "@loader_path/" + copied.name]
        for name, dependency in links[owner].items():
            prefix = "@executable_path/../Frameworks/" if copied == binary else "@loader_path/"
            edits += ["-change", name, prefix + sources[dependency].name]
        # Every non-system load command is now explicit and relative.
        for root in rpaths(copied):
            edits += ["-delete_rpath", root]
        if edits:
            run("/usr/bin/install_name_tool", *edits, str(copied))

    notices = resources / "native-video/ThirdPartyNotices"
    if notices.exists():
        shutil.rmtree(notices)
    notices.mkdir()
    packages = {}
    libraries = []
    for original_lib, copied in sources.items():
        if copied == binary:
            continue
        root = package_root(original_lib)
        key = f"{root.parent.name}-{root.name}"
        if key not in packages:
            notice_dir = notices / key
            notice_dir.mkdir()
            for item in root.iterdir():
                if item.is_file() and (item.name.upper().startswith(("COPYING", "LICENSE", "LICENCE", "COPYRIGHT", "NOTICE", "AUTHORS", "LGPL", "GPL"))
                                       or item.name in ("INSTALL_RECEIPT.json", "sbom.spdx.json")):
                    shutil.copy2(item, notice_dir / item.name)
            sbom = json.loads((root / "sbom.spdx.json").read_text())
            # Retain exact upstream source and patch references from the installed bottle.
            packages[key] = {"formula": root.parent.name, "version": root.name,
                             "sources": sbom.get("packages", [])}
        libraries.append({"name": copied.name, "package": key,
                          "original_sha256": digest(original_lib)})

    for plist in (app / "Contents/Info.plist", destination / "Contents/Info.plist"):
        data = plistlib.loads(plist.read_bytes())
        existing = tuple(map(int, data.get("LSMinimumSystemVersion", "11.0").split(".")))
        data["LSMinimumSystemVersion"] = ".".join(map(str, max(minimum, existing)))
        plist.write_bytes(plistlib.dumps(data))
    manifest = {"minimum_macos": ".".join(map(str, minimum)), "architectures": arch,
                "libraries": libraries, "packages": packages}
    manifest["version"] = json.loads((project / "tauri.conf.json").read_text())["version"]
    source_files = [project / name for name in ("Cargo.toml", "Cargo.lock", "build.rs", "tauri.conf.json", "macos/native_video.m")]
    for directory in ("src", "ui", "capabilities", "icons", "assets"):
        source_files.extend(path for path in (project / directory).rglob("*") if path.is_file() and path.name != ".DS_Store")
    manifest["source_sha256"] = {str(path.relative_to(project)): digest(path) for path in sorted(source_files)}
    (notices / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    shutil.copy2(project / "LICENSE", notices / "RustyViewer-LICENSE.txt")
    shutil.copy2(project / "THIRD_PARTY_NOTICES.md", notices / "README.md")
    shutil.copy2(project / "licenses/RustyViewer-MIT.txt", notices / "RustyViewer-original-MIT.txt")

    # Verify every rewritten load command before signing.
    for copied in sources.values():
        for name in dependencies(copied):
            if system_library(name):
                continue
            resolved = resolve(name, copied, binary)
            if not resolved.is_relative_to(destination.resolve()):
                raise RuntimeError(f"Dependency escaped the app: {name}")
        run("/usr/bin/codesign", "--force", "--sign", "-", str(copied))
    run("/usr/bin/codesign", "--force", "--sign", "-", str(destination))
    # Initialize the packaged engine and inspect what dyld actually loaded.
    for line in run(str(binary), "--check-bundle").splitlines():
        if not line.startswith("LIBRARY\t"):
            continue
        name = line.split("\t", 1)[1]
        if not system_library(name) and not Path(name).resolve().is_relative_to(destination.resolve()):
            raise RuntimeError(f"Runtime loaded a library outside the bundle: {name}")
    print(f"Bundled native player with {len(libraries)} libraries; minimum macOS {manifest['minimum_macos']}.")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("Usage: bundle_native_video.py /path/to/RustyViewer.app")
    bundle(Path(sys.argv[1]).resolve(strict=True))
