# RustyViewer morning review

RustyViewer is now a Tauri 2 desktop app. The release bundle is at:

`target/release/bundle/macos/RustyViewer.app`

Open that app first. For your first hands-on check, open a disposable image, change
brightness, rotate it, undo the rotation, and export a JPEG copy. You should see the
brightness retained after undo and a successful export message. Your source file
should remain unchanged.

## What changed

The egui window has been replaced with a dark, warm-accented workspace: folder
explorer on the left, zoomable canvas in the center, and adjustments on the right.
It includes native file dialogs, drag/drop, a batch studio, crop/resize dialogs,
keyboard help, busy feedback and visible error messages. Batch studio is also
available from the top bar when the window is too narrow for the explorer.

Rust still owns image processing and file writes. The frontend is plain HTML/CSS/
JavaScript, without a framework or frontend build requirement. The unused old
entry point and egui application module were removed; their originals remain in
Git history. The README describes the new layout and build workflow. Changes are
left uncommitted for review.

## Bugs and safeguards addressed

- Out-of-bounds cropping could subtract past zero. Bounds are checked first.
- Saving edited/transparent images as JPEG could fail because JPEG cannot encode
  alpha. Exports now composite transparency onto white and encode RGB.
- Open/save/clipboard failures were only printed in the terminal. They now reach
  visible error messages in the interface.
- Navigating, opening or pasting another image could silently lose edits. These
  transitions and quitting now prompt for unexported changes.
- Undo only stored base pixels. History now includes sliders and supports redo;
  pixel operations act on the image as displayed, and undo restores that state.
- Batch output could overwrite input images or other files with the same name.
  Batch writes now refuse to replace existing files, catch conflicting outputs,
  continue after individual failures and summarize the results.
- Batch proportional resize used only width and could exceed the selected height.
  It now fits inside both dimensions.
- Image navigation now retains extensionless decoded images and deduplicates
  canonical paths, with deterministic case-insensitive sorting.
- EXIF orientation is applied when loading photographs.
- Resize/adjustment arguments are validated before use; history and dimensions
  have explicit limits to reduce accidental excessive memory use.
- Save operations encode into a temporary file before replacing a destination,
  reducing the risk of truncating a file after an encoding failure.
- Clipboard ownership remains alive for Linux clipboard behavior.
- Finder open events received before interface initialization are retained.
- Focused buttons keep standard Enter/Space keyboard activation.

## Verification completed

- `cargo test --offline --locked`: **16 tests passed** (6 existing/core tests,
  9 document/file regressions and 1 Tauri command integration test).
- The command integration test invokes open, adjust, rotate, undo and JPEG export
  through Tauri's IPC test runtime, and verifies failed opens are errors.
- `cargo clippy --offline --all-targets -- -D warnings`: passed.
- `cargo fmt --check`, `npm run check`, `npm run format:check` and
  `git diff --check`: passed.
- A native debug launch initialized all frontend listeners and opened the
  repository's 512×512 icon via the real Tauri frontend-to-Rust connection.
- `./macos/build_app.sh`: release app built; `codesign --verify --deep --strict`
  passed. The build contains the interface and needs no running development server.

## Still needs your hands-on review

Automated native mouse/keyboard control is unavailable in this session. The browser
preview could not reach the local server, and browser policy blocked opening the
HTML directly from disk. I did not work around that restriction, so I cannot claim
visual screenshot review or a complete interactive test of every control.

After the first open/edit/undo/export check, the remaining useful checks are native
Open/Save dialogs, dragging a file from Finder, crop selection while zoomed/panned,
clipboard copy/paste, the discard/keep-editing prompts, Finder's Open With, and a
small batch into an empty temporary folder. Repeating that batch should skip the
outputs, not overwrite them. Try resizing the window to check the layout.

Only macOS was built and launched here. Linux/Windows runtime behavior needs testing
on those platforms. The local app is ad-hoc signed, not Developer ID signed or
notarized. GIF/TIFF/ICO remain single-frame/page views; original EXIF/ICC metadata
is not carried into exports. Adjustments update when the slider is released; large
images can take time to render at full resolution.
