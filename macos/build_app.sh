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
FLAGS=()
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="debug"
    FLAGS+=(--debug)
elif [[ -n "${1:-}" ]]; then
    echo "Usage: ./macos/build_app.sh [--debug]" >&2
    exit 1
fi
cargo tauri build --bundles app "${FLAGS[@]}" -- --locked
APP_PATH="$PROJECT_ROOT/target/$PROFILE/bundle/macos/RustyViewer.app"
codesign --force --deep --sign - "$APP_PATH"
codesign --verify --deep --strict "$APP_PATH"
echo "Built: $APP_PATH"
echo "Local development signature only; not Developer ID signed or notarized."
