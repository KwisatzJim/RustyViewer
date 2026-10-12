#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"
if [[ "$(uname -s)" != Linux || $# -ne 0 ]]; then
    echo 'Usage on Linux: ./linux/build_app.sh' >&2
    exit 1
fi
./linux/test_native_video.sh --build
if [[ -x "$PROJECT_ROOT/node_modules/.bin/tauri" ]]; then
    "$PROJECT_ROOT/node_modules/.bin/tauri" build --bundles deb -- --locked
else
    echo "Run npm ci first to install the locked Tauri CLI." >&2
    exit 1
fi
echo "Built Ubuntu/Debian installer in target/release/bundle/deb/"
