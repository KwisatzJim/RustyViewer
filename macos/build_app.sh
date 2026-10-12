#!/usr/bin/env bash
# Build the Tauri app with its embedded interface and Finder file associations.
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"
if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "This script builds macOS apps. Use cargo tauri build on your platform." >&2
    exit 1
fi
PROFILE="release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
elif [[ -n "${1:-}" ]]; then
    echo "Usage: ./macos/build_app.sh [--debug]" >&2
    exit 1
fi
if [[ -x "$PROJECT_ROOT/node_modules/.bin/tauri" ]]; then
    TAURI=("$PROJECT_ROOT/node_modules/.bin/tauri")
elif cargo tauri --version >/dev/null 2>&1; then
    TAURI=(cargo tauri)
else
    echo "Tauri CLI not found. Run 'npm ci' in the project first." >&2
    exit 1
fi

./macos/test_native_video.sh --build
if [[ "$PROFILE" == "debug" ]]; then
    "${TAURI[@]}" build --bundles app --debug -- --locked
else
    "${TAURI[@]}" build --bundles app -- --locked
fi
APP_PATH="$PROJECT_ROOT/target/$PROFILE/bundle/macos/RustyViewer.app"
python3 "$PROJECT_ROOT/macos/bundle_native_video.py" "$APP_PATH"
codesign --force --deep --sign - "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"
echo "Built: $APP_PATH"
echo "Local development signature only; not Developer ID signed or notarized."
