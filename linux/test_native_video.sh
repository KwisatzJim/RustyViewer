#!/usr/bin/env bash
set -euo pipefail
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "$(uname -s)" != Linux || $# -gt 1 ]]; then
    echo 'Usage on Linux: ./linux/test_native_video.sh [video-path | --build]' >&2
    exit 1
fi
if ! pkg-config --exists mpv gtk+-3.0 epoxy; then
    echo 'Install libmpv-dev and libgtk-3-dev first.' >&2
    exit 1
fi
mkdir -p "$PROJECT_ROOT/target/native-video"
BINARY="$PROJECT_ROOT/target/native-video/rustyviewer-video"
read -r -a PLAYER_FLAGS <<< "$(pkg-config --cflags --libs mpv gtk+-3.0 epoxy)"
cc -std=c11 -Wall -Wextra -Werror "$PROJECT_ROOT/linux/native_video.c" \
    "${PLAYER_FLAGS[@]}" -ldl -o "$BINARY"
if [[ "${1:-}" == --build ]]; then
    echo "Built: $BINARY"
elif [[ $# -eq 1 ]]; then
    exec "$BINARY" "$1"
else
    exec "$BINARY"
fi
