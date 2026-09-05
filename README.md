# RustyViewer

[![CI](https://github.com/KwisatzJim/RustyViewer/actions/workflows/ci.yml/badge.svg)](https://github.com/KwisatzJim/RustyViewer/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/KwisatzJim/RustyViewer)](https://github.com/KwisatzJim/RustyViewer/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A focused, local-first image viewer and editor inspired by IrfanView. RustyViewer
uses Rust and Tauri 2 and keeps image processing on your computer. It has no account,
telemetry, advertising, or remote image service.

## Install

Download the installer for your operating system from the
[latest GitHub release](https://github.com/KwisatzJim/RustyViewer/releases/latest):

- **macOS Apple Silicon:** download the `aarch64` DMG.
- **macOS Intel:** download the `x86_64` DMG.
- **Windows:** download the NSIS setup `.exe` or MSI installer.
- **Linux:** download the x64 AppImage or Debian package.

The macOS and Windows packages are not yet signed with paid distribution
certificates. Your operating system may ask you to confirm that you trust the
download. On macOS, open **System Settings → Privacy & Security** after the first
blocked launch and choose **Open Anyway**. On Windows, inspect the SmartScreen
publisher warning before choosing **Run anyway**.

For an AppImage:

```sh
chmod +x RustyViewer_*.AppImage
./RustyViewer_*.AppImage
```

## Build from source

Install Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
Then clone the repository and install its locked development tools:

```sh
git clone https://github.com/KwisatzJim/RustyViewer.git
cd RustyViewer
npm ci
npm run tauri dev
```

Build an installer with `npm run tauri build`. The finished app embeds its HTML,
CSS and JavaScript and does not require Node.js.

### macOS

```sh
./macos/build_app.sh
```

The result is `target/release/bundle/macos/RustyViewer.app`. Add `--debug` for a
faster development bundle. The script applies an ad-hoc signature and verifies it;
it does not create a Developer ID signed or notarized distribution.

### Linux / Windows

Run `npm run tauri build` on the target platform. On Linux, Tauri 2 needs WebKitGTK
4.1 and GTK development libraries (see the prerequisites above).

```sh
NO_STRIP=1 npm run tauri build -- --bundles appimage
```

Use that command when building an AppImage on an Arch-based distribution.

## Features

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
- `src/navigation.rs`: folder scanning and supported image types.
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

RustyViewer is available under the [MIT License](LICENSE).
