#!/usr/bin/env bash
#
# Build a real, double-clickable RustyViewer.app bundle on macOS.
#
# Usage:
#   ./macos/build_app.sh            # release build (default)
#   ./macos/build_app.sh --debug    # debug build, faster iteration
#
# Requires Xcode Command Line Tools (for `iconutil` and `codesign`), which
# you almost certainly already have if you've built anything on this Mac.
# Run `xcode-select --install` if either command is missing.
#
# This is macOS-only by nature (iconutil/codesign don't exist elsewhere);
# it has no effect on your normal `cargo build`/`cargo run` on Linux.

set -euo pipefail

APP_NAME="RustyViewer"
BIN_NAME="rusty_viewer" # must match the `name` in Cargo.toml
BUNDLE_ID="com.jim.rustyviewer" # change to your own reverse-DNS id if you like

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ASSETS_DIR="$PROJECT_ROOT/assets"

BUILD_PROFILE="release"
CARGO_FLAGS=(--release)
if [[ "${1:-}" == "--debug" ]]; then
    BUILD_PROFILE="debug"
    CARGO_FLAGS=()
fi

for tool in iconutil codesign; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "error: '$tool' not found. Install the Xcode Command Line Tools with:" >&2
        echo "  xcode-select --install" >&2
        exit 1
    fi
done

echo "==> Building $APP_NAME ($BUILD_PROFILE)..."
(cd "$PROJECT_ROOT" && cargo build "${CARGO_FLAGS[@]}")

BIN_PATH="$PROJECT_ROOT/target/$BUILD_PROFILE/$BIN_NAME"
if [[ ! -f "$BIN_PATH" ]]; then
    echo "error: built binary not found at $BIN_PATH" >&2
    exit 1
fi

APP_DIR="$PROJECT_ROOT/target/$BUILD_PROFILE/bundle/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"

echo "==> Assembling bundle at $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$MACOS_DIR" "$RESOURCES_DIR"

cp "$BIN_PATH" "$MACOS_DIR/$APP_NAME"
chmod +x "$MACOS_DIR/$APP_NAME"

echo "==> Building AppIcon.icns from assets/icon-*.png"
ICONSET_PARENT="$(mktemp -d)"
ICONSET_DIR="$ICONSET_PARENT/AppIcon.iconset"
mkdir -p "$ICONSET_DIR"

# iconutil requires this exact filename set (standard sizes + @2x retina variants).
cp "$ASSETS_DIR/icon-16.png"   "$ICONSET_DIR/icon_16x16.png"
cp "$ASSETS_DIR/icon-32.png"   "$ICONSET_DIR/icon_16x16@2x.png"
cp "$ASSETS_DIR/icon-32.png"   "$ICONSET_DIR/icon_32x32.png"
cp "$ASSETS_DIR/icon-64.png"   "$ICONSET_DIR/icon_32x32@2x.png"
cp "$ASSETS_DIR/icon-128.png"  "$ICONSET_DIR/icon_128x128.png"
cp "$ASSETS_DIR/icon-256.png"  "$ICONSET_DIR/icon_128x128@2x.png"
cp "$ASSETS_DIR/icon-256.png"  "$ICONSET_DIR/icon_256x256.png"
cp "$ASSETS_DIR/icon-512.png"  "$ICONSET_DIR/icon_256x256@2x.png"
cp "$ASSETS_DIR/icon-512.png"  "$ICONSET_DIR/icon_512x512.png"
cp "$ASSETS_DIR/icon-1024.png" "$ICONSET_DIR/icon_512x512@2x.png"

iconutil --convert icns "$ICONSET_DIR" --output "$RESOURCES_DIR/AppIcon.icns"
rm -rf "$ICONSET_PARENT"

echo "==> Writing Info.plist"
VERSION="$(grep -m1 '^version' "$PROJECT_ROOT/Cargo.toml" | sed -E 's/version *= *"([^"]+)".*/\1/')"

cat > "$CONTENTS_DIR/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleExecutable</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.graphics-design</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Image</string>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
            <key>LSHandlerRank</key>
            <string>Alternate</string>
            <key>LSItemContentTypes</key>
            <array>
                <string>public.image</string>
                <string>public.png</string>
                <string>public.jpeg</string>
                <string>public.tiff</string>
                <string>com.compuserve.gif</string>
            </array>
        </dict>
    </array>
</dict>
</plist>
PLIST

echo "==> Ad-hoc signing (avoids Gatekeeper refusing to launch an unsigned local build)"
codesign --force --deep --sign - "$APP_DIR"

echo "==> Done: $APP_DIR"
echo "    Open it with: open \"$APP_DIR\""
echo "    Or drag it into /Applications from Finder."