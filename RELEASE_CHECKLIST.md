# Release checklist

1. Choose the next semantic version and update `Cargo.toml`, `tauri.conf.json`,
   `package.json`, `package-lock.json` and `CHANGELOG.md`.
2. Run `npm run version:check` and all checks listed in `README.md`.
3. Test open, drag/drop, adjust, rotate, undo/redo, crop, resize, clipboard, export,
   unsaved-change prompts and Batch studio in a real Tauri window.
4. For Batch studio, process disposable images into an empty folder, then repeat the
   batch and confirm that existing outputs are skipped.
5. Commit and push the release changes. Wait for the `CI` workflow to pass on `main`.
6. Create and push a tag that exactly matches the version, for example `0.2.1`.
7. Wait for every job in the `Release` workflow. It creates a **draft** GitHub release.
   The Apple Silicon Mac job uses the native bundling scripts and attaches a
   matching source archive and checksums. Intel Mac packages are paused until
   native libraries have been built and tested on that architecture.
8. Download each artifact, check its name and size, and install packages on the
   platforms available to you.
9. Edit the draft notes if needed, then publish the release from GitHub.

The release workflow deliberately stops at a draft. Publishing remains an explicit
maintainer action after the generated packages have been reviewed.

## Current signing status

macOS builds use an ad-hoc signature, so Gatekeeper can require **Open Anyway**.
Windows builds are unsigned and can show SmartScreen warnings. Do not describe these
packages as Developer ID signed, notarized, or Authenticode signed.

## Mac native-video checks

- Build with `./macos/build_app.sh`, then `./macos/package_dmg.sh`.
- Collect sources with `python3 macos/collect_video_sources.py --download`,
  `cargo vendor --locked target/release/rust-vendor`, and
  `python3 macos/package_sources.py`. The packager rejects stale application
  sources by comparing them with the fingerprints recorded in the app.
- Verify both archive checksum manifests, mounted disk-image contents and
  `codesign --verify --deep --strict`.
- Run the bundled helper's `--check-bundle`; no non-system libraries should load
  from outside the application. Check a relocated copy and MKV playback,
  subtitle visibility, seeking, audio, and closing/reopening.
- Publish the matching source archive alongside the binary, review
  `macos/SOURCE_BUILD.md`, and record the fresh-library-rebuild validation limit.
- State the minimum macOS version recorded in that particular app. This local
  0.3.0 Apple Silicon build requires macOS 27; CI builds can use different libraries.
