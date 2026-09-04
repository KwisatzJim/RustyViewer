# RustyViewer

A local image viewer and editor inspired by IrfanView, built with Rust and Tauri 2.
The interface uses plain HTML, CSS and JavaScript: there is no frontend framework,
remote service, account, or Node runtime in the finished desktop app.

## Run

Install Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/).
Then install the Tauri CLI once:

```sh
cargo install tauri-cli --version '^2' --locked
```

From this repository:

```sh
cargo tauri dev
```

You can also use `cargo run -- path/to/image.png`. The interface is embedded when
building; `cargo tauri dev` provides the development reload workflow.

### Build a macOS app

```sh
./macos/build_app.sh
```

The result is `target/release/bundle/macos/RustyViewer.app`. Double-click it or copy
it to Applications. For a faster development bundle, add `--debug`.
The script applies a local ad-hoc signature; it does not produce a Developer ID
signed or notarized distribution. File associations support Finder's **Open With**.

### Linux / Windows

Run `cargo tauri build` on the target platform. On Linux, Tauri 2 needs WebKitGTK
4.1 and GTK development libraries (see the prerequisites above). The old egui/X11
workaround has been removed; Tauri provides the native window and drag/drop events.
Linux and Windows builds should be verified on those platforms before distribution.

## What you can do

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

## How the project fits together

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
```

Node/npm are only needed for these optional syntax/formatting checks. `npm run
format` formats the interface source. Rust tests use temporary fixtures and do not
modify personal photos. See `MORNING_REVIEW.md` for the migration review and the
remaining manual checks.
