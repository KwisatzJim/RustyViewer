# Changelog

## [0.3.0] - 2026-10-11

### Added

- Video playback with folder navigation alongside images.
- Native playback for all recognized videos on macOS and Linux using libmpv, with seeking,
  volume, fullscreen and subtitle visibility controls.
- Mac disk-image packaging and source collection with checksum verification,
  pinned library build recipes and vendored Rust dependencies.

### Changed

- RustyViewer is now GPL-3.0-or-later; original MIT notices remain included.
- The current Apple Silicon Mac package requires macOS 27 because of its
  bundled library builds. It uses a local signature and is not notarized.
- Linux native video integration is tested on Ubuntu 26.04; Intel Mac packaging remains unverified.

## [0.2.1] - 2026-09-05

### Added

- Automated checks for macOS, Linux and Windows changes.
- Draft releases with Apple Silicon and Intel DMGs, Linux AppImage and DEB packages,
  and Windows NSIS and MSI installers.
- Contribution, private security-reporting, issue-template and release documentation.
- A version-consistency check for Cargo, Tauri and npm manifests.

### Changed

- Updated package metadata and Linux desktop integration for public distribution.
- Configured macOS packages with an ad-hoc signature until Developer ID signing is available.

## [0.2.0] - 2026-09-03

### Added

- Tauri 2 interface with folder browsing, adjustments, quick actions and Batch studio.
- Undo/redo, safe crop and resize validation, EXIF orientation, native clipboard support
  and visible operation errors.
- Atomic exports and non-overwriting batch processing with per-file results.

### Changed

- Replaced the original egui interface with an embedded HTML, CSS and JavaScript frontend.

