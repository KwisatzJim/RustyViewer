# Native video components

RustyViewer uses libmpv for native video playback on macOS and Linux. The macOS build script
copies libmpv and its linked libraries into the application, rewrites their load
paths, and signs those copies. System libraries remain provided by macOS.

The bundled software retains its own copyright and license terms. These include
GPL, LGPL, BSD, MIT and other licenses; the combined RustyViewer distribution is
provided under GPL-3.0-or-later. Earlier MIT notices are retained in
`licenses/RustyViewer-MIT.txt`.

Each macOS bundle contains `Contents/Resources/native-video/ThirdPartyNotices`,
with the installed libraries' license files, Homebrew installation receipts,
SPDX provenance documents and a manifest recording versions, original binary
hashes, upstream source URLs and patches. The manifest describes the exact
libraries bundled in that build, rather than an assumed dependency list.

Before publishing binaries, distribute their complete corresponding source,
including applicable patches and build instructions, alongside the release.
The upstream links and provenance manifest help prepare that source package;
they are not themselves a replacement for the corresponding source. Build and
package RustyViewer from the matching revision and retain its Cargo.lock.

Run `python3 macos/collect_video_sources.py` to inventory the upstream archives
and patches recorded in the release app. Add `--download` to collect them into
`target/release/video-sources` and verify each SHA-256 checksum. Git sources are
collected as self-contained Git bundles pinned to the revision recorded in the
installation receipt. The installed Homebrew recipes and receipts are retained
under `recipes`. Existing files are verified again, so an interrupted collection
can be resumed. This directory is a staging area. Run
`cargo vendor --locked target/release/rust-vendor`, then
`python3 macos/package_sources.py` to assemble the matching application source,
Rust dependencies, library sources, recipes and build instructions into an
archive. The packager checks source fingerprints recorded in the app and emits
checksums for the archive and its contents. See `macos/SOURCE_BUILD.md` for
collection coverage and the unverified fresh-library-rebuild limitation.

Homebrew is required on the build machine, but is not needed to run the finished
macOS bundle. The minimum macOS version is derived from the copied libraries;
building for older Macs requires compatible versions/builds of those libraries.

Project information:
- mpv: https://mpv.io/
- FFmpeg: https://ffmpeg.org/
- Homebrew: https://brew.sh/
