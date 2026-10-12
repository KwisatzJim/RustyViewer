#!/usr/bin/env bash
# Build and launch the standalone macOS libmpv experiment.
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != Darwin || $# -gt 1 ]]; then
    echo 'Usage on macOS: ./macos/test_native_video.sh [video-path | --build]' >&2
    exit 1
fi
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
if ! pkg-config --exists mpv; then
    echo 'Install libmpv first: brew install mpv pkgconf' >&2
    exit 1
fi
OUTPUT="$PROJECT_ROOT/target/native-video"
APP="$OUTPUT/RustyViewer Video.app"
mkdir -p "$APP/Contents/MacOS"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>rustyviewer-video</string>
<key>CFBundleIdentifier</key><string>com.jim.rustyviewer.video-prototype</string>
<key>CFBundleName</key><string>RustyViewer Video</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
BINARY="$APP/Contents/MacOS/rustyviewer-video"
read -r -a MPV_FLAGS <<< "$(pkg-config --cflags --libs mpv)"
clang -fobjc-arc -Wall -Wextra -Werror -Wno-deprecated-declarations \
    "$PROJECT_ROOT/macos/native_video.m" "${MPV_FLAGS[@]}" \
    -framework Cocoa -framework OpenGL -o "$BINARY"
if [[ "${1:-}" == --build ]]; then
    echo "Built: $BINARY"
elif [[ $# -eq 1 ]]; then
    exec "$BINARY" "$1"
else
    exec "$BINARY"
fi
