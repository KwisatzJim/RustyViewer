# Mac source package build and audit

This package includes the working RustyViewer source (including uncommitted changes),
Cargo.lock, the upstream source archives and patches recorded in the Mac bundle,
Git bundles, the 41 installed Homebrew recipes, two repository-local patches,
vendored Rust dependencies, and bundle provenance.
SHA256SUMS records each packaged file. The enclosing archive has its own checksum.

## Build RustyViewer

Use an Apple Silicon Mac with Xcode command-line tools, Rust supporting edition
2024, Node.js/npm, Python 3 and Homebrew. The libraries used in this snapshot
require macOS 27. Install the Tauri macOS prerequisites and run, in RustyViewer:

```sh
npm ci
brew install mpv pkgconf
./macos/build_app.sh
./macos/package_dmg.sh
```

These commands build against the installed libraries. Rust builds can use the
packaged vendor directory without fetching crates:

```sh
cargo build --release --locked --offline --features custom-protocol
```

The generated .cargo/config.toml selects this directory. Node.js and the Tauri
CLI are build tools, not runtime dependencies. `npm ci` needs network access;
alternatively install the Tauri CLI separately with Cargo.

To reconstruct the recorded library versions instead of current Homebrew
versions, prepare a local Homebrew tap from the packaged sources:

```sh
python3 RustyViewer/macos/prepare_source_tap.py video-sources /absolute/path/source-tap
brew tap rustyviewer/sources /absolute/path/source-tap
brew install --build-from-source rustyviewer/sources/mpv
```

Run these commands from the package root on a fresh build machine or disposable
Homebrew installation. Avoid replacing libraries in your everyday Homebrew
installation. The helper verifies all source pins, prepares local file URLs,
and directs dependencies with retained recipes to the local tap. General build
tools and dependencies outside the bundled library set are resolved by Homebrew
and may need network access. A full library rebuild has not been tested on a
fresh Mac. The retained recipes' install methods record configure flags,
patching steps and dependencies. The receipts record installed versions and
the compiler/SDK environment. Upstream archives retain their original compression
format although their filenames use the provenance identifiers. Use `file` to
identify the archive format before extracting. Apply patches in the recipe's
order. For x264, obtain its recorded source without a network connection:

```sh
git clone --branch source video-sources/x264-r3222/SPDXRef-Archive-x264-src.bundle x264
```

The recorded revision is b35605ace3ddf7c1a5d67a2eb553f034aef41d55.
Bundling copies libraries, changes their load paths and signs the app locally;
it does not provide Developer ID signing or notarization.

## Collection coverage and validation limits

- Seven additional build resources are now collected: shaderc's glslang,
  SPIRV-Headers and SPIRV-Tools Git resources, libplacebo's glad2, jinja2 and
  markupsafe archives, and GLib's gobject-introspection archive. Git bundles
  can be cloned with `--branch source`; inventory.json records their revisions.
- Both repository-local patches named by the GLib and libb2 recipes are included.
  Their pinned repository revision, SHA-256 hashes and last-change records are
  recorded under video-sources/homebrew-patches. Their last changes predate the
  recorded library builds.
- Inline patches and source substitutions remain in the retained recipes.
- Test-only recipe fixtures are not included; these do not participate in
  building the libraries shipped in the application.
- This package does not promise bit-identical Homebrew bottle reconstruction.
  A fresh rebuild of the entire library chain remains a separate validation
  step before claiming reproducibility. Apple system libraries and general
  compilers/build tools are supplied by their respective platforms.

The package verification command is:

```sh
python3 macos/verify_packages.py --offline-build
```

Run it in the build checkout after preparing the app, disk image and source
archive. It verifies the mounted app, native-library locations, every archived
file checksum and the source fingerprints. It then builds RustyViewer from an
extracted source archive with an empty Cargo cache and Cargo offline mode.
This validates the application and Rust dependencies; it uses the host's system
libraries and does not rebuild the full Homebrew library chain.
