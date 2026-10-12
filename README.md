# RustyViewer

[![CI](https://github.com/KwisatzJim/RustyViewer/actions/workflows/ci.yml/badge.svg)](https://github.com/KwisatzJim/RustyViewer/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/KwisatzJim/RustyViewer)](https://github.com/KwisatzJim/RustyViewer/releases/latest)
[![License: GPLv3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)

A focused, local-first image viewer and editor inspired by IrfanView. RustyViewer
uses Rust and Tauri 2 and keeps image processing on your computer. It has no account,
telemetry, advertising, or remote image service.

## Install

Download the installer for your operating system from the
[latest GitHub release](https://github.com/KwisatzJim/RustyViewer/releases/latest):

- **macOS Apple Silicon:** use the `aarch64` DMG. The locally tested 0.3.0 build
  requires macOS 27. It includes libmpv and its libraries; Homebrew is not required
  on the user's Mac. CI-built packages may have a different minimum macOS version.
- **Linux x64:** use the DEB and install it with `sudo apt install ./package.deb`.
  This resolves its system-library dependencies. The local package was tested on
  Ubuntu 26.04; the automated release build uses Ubuntu 24.04 and needs separate
  installation testing.
- **Windows:** NSIS `.exe` and MSI packages are built by the release workflow.
  Native libmpv playback is currently implemented on Mac and Linux; Windows
  video playback uses the webview and depends on its supported codecs.

Intel Mac and Linux AppImage packages are paused for 0.3.0 until their native
video packaging has been verified. Recognition currently covers MP4, M4V, MOV,
WebM, OGV, MKV and AVI. A file's extension does not guarantee that its particular
encoding can be decoded.

The Mac app has a local ad-hoc signature and is **not Developer ID signed or
notarized**. On a blocked launch, review **System Settings → Privacy & Security**
and choose **Open Anyway** if you trust the download. Windows installers are
unsigned and can show a SmartScreen warning.

Matching source archives and SHA-256 checksums accompany the release. Mac native
library sources, patches and recipes are included in the Mac source archive.
The application source archive includes Rust dependency sources for building
RustyViewer on Linux and Windows. Linux uses the distribution's libmpv package.

## Build from source

Install Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
Then clone the repository and install its locked development tools:

```sh
git clone https://github.com/KwisatzJim/RustyViewer.git
cd RustyViewer
npm ci
# Build the native helper first, using the instructions for your platform below.
npm run tauri dev
```

Build macOS and Linux with their platform scripts below to include native video
playback. On Windows, build an installer with `npm run tauri build`. The finished app embeds its HTML,
CSS and JavaScript and does not require Node.js.

### macOS

```sh
brew install mpv pkgconf
./macos/build_app.sh
```

The result is `target/release/bundle/macos/RustyViewer.app`. Add `--debug` for a
faster development bundle. The script applies an ad-hoc signature and verifies it;
it does not create a Developer ID signed or notarized distribution.

After building the release app, create a disk image with:

```sh
./macos/package_dmg.sh
```

The disk image and its SHA-256 checksum are written to
`target/release/bundle/dmg/`. This packages the completed app, including its native
video libraries and notices. Prepare the corresponding source package described
in `THIRD_PARTY_NOTICES.md` before publishing binaries.

Prepare the matching source archive after the app build:

```sh
python3 macos/collect_video_sources.py --download
cargo vendor --locked target/release/rust-vendor
python3 macos/package_sources.py
python3 macos/verify_packages.py --offline-build
```

The archive includes the source snapshot, Rust dependencies, native library
sources, patches, recipes, provenance and a checksum manifest. Build instructions
and validation limits are in `macos/SOURCE_BUILD.md`. A fresh rebuild of the full
library chain has not yet been verified. The archive and checksum are written to
`target/release/bundle/sources/`.

### Native video playback (macOS and Linux)

All recognized videos open in a separate libmpv window on macOS and Linux.
Builds made with `./macos/build_app.sh` include the Mac native player and its
libraries, so users do not need Homebrew. Homebrew is needed on the build machine.
The bundle’s minimum macOS version follows its bundled libraries; the current
Homebrew libraries on this build machine require macOS 27.

```sh
brew install mpv pkgconf
./macos/test_native_video.sh --build
npm run tauri dev
```

Open or drop a video in RustyViewer. Use the video window’s controls for pause,
seeking, volume and fullscreen. The **Subtitles** checkbox shows or hides the
selected subtitle track. Opening another file closes the previous native
window; **Reopen video window** starts the current video again. To test the helper
on its own, run `./macos/test_native_video.sh` and choose a video.

### Linux (Ubuntu/Debian)

Install the Tauri Linux prerequisites and `libmpv-dev`, `libgtk-3-dev` and
`libepoxy-dev`, then run:

```sh
npm ci
./linux/build_app.sh
```

The DEB is written to `target/release/bundle/deb/`. It includes the native helper
and declares system video-library dependencies. Install with `sudo apt install
./path/to/package.deb` so missing dependencies are resolved. A package built on
Ubuntu 26.04 is intended for that system; compatibility with older distributions
has not been verified.

For Linux development, build the helper before opening the app:

```sh
./linux/test_native_video.sh --build
npm run tauri dev
```

### Windows

Run `npm run tauri build` to create NSIS and MSI installers. Windows currently
uses the webview for video playback and has no bundled libmpv helper.

Create the application source archive for Linux and Windows with:

```sh
cargo vendor --locked target/release/rust-vendor
python3 scripts/package-application-sources.py
```

Its Cargo configuration selects the included dependency sources. Building with
`cargo build --release --locked --offline --features custom-protocol` does not
fetch Rust crates; platform development libraries and build tools are still
required.

## Features

- Play MP4, M4V, MOV, WebM, OGV, MKV and AVI videos through libmpv on macOS and Linux, with playback, seeking, volume, fullscreen and a subtitle visibility checkbox. Windows uses the webview player where supported. Video editing is not supported.
- Open or drop PNG, JPEG, WebP, GIF, BMP, TIFF and ICO images.
- Browse neighboring files in the folder explorer or with the arrow keys.
- Zoom, fit to the window, view at 100%, and drag to pan.
- Adjust brightness, contrast, saturation, gamma and RGB tint.
- Rotate, flip, grayscale, invert and auto-adjust colors.
- Crop by selecting an area, then clicking **Apply crop** or pressing Enter.
- Resize with optional aspect-ratio locking and Lanczos resampling.
- Undo and redo edits, including slider adjustments.
- Copy and paste images through the system clipboard.
- Export PNG, JPEG, WebP, BMP or TIFF using the native save dialog.
- Convert collections in **Batch studio**, with resizing and optional filters.
- See keyboard shortcuts using the bottom-right button or `?`.

Slider changes are applied when you release the slider (or complete a keyboard
change). Rust runs image operations off the interface thread. Previews retain full
resolution so **100%** displays actual pixels, and exports use the full image.
Large images can take a moment to update; a busy indicator shows the operation.

## File safety and limits

Your source image is unchanged unless you deliberately export over it. Export
suggests an `-edited.png` copy. File writes encode to a temporary file in the
output folder and replace the destination only after encoding succeeds.
Unexported edits trigger a confirmation before navigating, opening, pasting or
quitting. Saving an exported copy marks that revision as exported; the folder
explorer continues to show the original source directory.

Batch processing never replaces existing files. It skips output-name conflicts,
continues after individual failures and reports every skipped or failed input.
When preserving proportions, batch resize fits inside **both** supplied dimensions.

JPEG exports composite transparent pixels onto white. Other supported exports
preserve alpha where the format supports it. EXIF orientation is applied on load.
Exports contain re-encoded pixels; original EXIF, ICC and other metadata are not
preserved. GIF/TIFF/ICO are viewed and edited as a single decoded frame/page/image;
this is not an animation or multipage editor. Images and requested resizes are
limited to 80 megapixels. Undo retains at most 20 prior revisions, trimming older
pixel history around a 256 MiB budget (at least one previous revision remains).

## Architecture and development

- `ui/index.html`: window structure, tool panels and dialogs.
- `ui/styles.css`: layout, colors, controls and responsive sizing.
- `ui/app.js`: interface state, keyboard/drag controls and calls into Rust.
- `src/main.rs`: the Tauri application, native events and command boundary.
- `src/editor.rs`: document revisions, decoding, export and batch processing.
- `src/image_ops.rs`: reusable image adjustments and transformations.
- `src/navigation.rs`: folder scanning and supported image/video types.
- `macos/native_video.m` and `linux/native_video.c`: platform video windows using libmpv.
- `tauri.linux.conf.json`: native helper resources and DEB runtime dependencies.
- `src/settings.rs`: remembers the last batch output directory.
- `tauri.conf.json` and `capabilities/main.json`: packaging and allowed native APIs.

The JavaScript interface sends a request such as `edit_image` to Rust. Rust
validates it, changes the in-memory document, then returns its image and metadata.
The interface never writes image files itself. Tests also exercise this command
boundary to catch mismatches between the frontend and Rust.

## Checks

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
```

Optional JavaScript development tools:

```sh
npm ci
npm run check
npm run format:check
npm run version:check
```

Rust tests use temporary fixtures and do not modify personal photos. Changes to
visible interactions must also be checked in a real Tauri window because a browser
preview cannot exercise the native command bridge.

See [CONTRIBUTING.md](CONTRIBUTING.md) to propose a change,
[SECURITY.md](SECURITY.md) to report a vulnerability privately, and
[CHANGELOG.md](CHANGELOG.md) for release history.

RustyViewer is available under [GPL-3.0-or-later](LICENSE). See
[third-party notices](THIRD_PARTY_NOTICES.md) for bundled video components.
