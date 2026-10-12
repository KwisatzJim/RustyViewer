#!/usr/bin/env bash
# Package the completed app, after native-video bundling and signing.
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$PROJECT_ROOT/target/release/bundle/macos/RustyViewer.app"
if [[ "$(uname -s)" != Darwin || $# -ne 0 ]]; then
    echo 'Usage on macOS: ./macos/package_dmg.sh' >&2
    exit 1
fi
if [[ ! -f "$APP/Contents/Resources/native-video/ThirdPartyNotices/manifest.json" ]]; then
    echo 'Build the bundled release app first: ./macos/build_app.sh' >&2
    exit 1
fi
codesign --verify --deep --strict "$APP"
VERSION="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$PROJECT_ROOT/tauri.conf.json")"
APP_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist")"
if [[ "$APP_VERSION" != "$VERSION" ]]; then
    echo "App version $APP_VERSION differs from project version $VERSION; rebuild first." >&2
    exit 1
fi
ARCH="$(lipo -archs "$APP/Contents/MacOS/rusty_viewer")"
case "$ARCH" in
    arm64) LABEL=aarch64 ;;
    x86_64) LABEL=x86_64 ;;
    "arm64 x86_64"|"x86_64 arm64") LABEL=universal ;;
    *) echo "Unsupported app architecture: $ARCH" >&2; exit 1 ;;
esac
STAGING="$(mktemp -d "${TMPDIR:-/private/tmp}/RustyViewer-dmg.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
ditto "$APP" "$STAGING/RustyViewer.app"
ln -s /Applications "$STAGING/Applications"
OUTPUT="$PROJECT_ROOT/target/release/bundle/dmg"
mkdir -p "$OUTPUT"
DMG="$OUTPUT/RustyViewer_${VERSION}_${LABEL}.dmg"
hdiutil create -volname RustyViewer -srcfolder "$STAGING" -format UDZO \
    -fs HFS+ -ov "$DMG"
hdiutil verify "$DMG"
(
    cd "$OUTPUT"
    shasum -a 256 "RustyViewer_${VERSION}_${LABEL}.dmg" > "RustyViewer_${VERSION}_${LABEL}.dmg.sha256"
)
echo "Built: $DMG"
