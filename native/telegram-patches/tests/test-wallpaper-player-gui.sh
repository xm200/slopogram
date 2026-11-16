#!/bin/bash
set -euo pipefail
root=$(cd "$(dirname "$0")/../../.." && pwd)
tmp=$(mktemp -d "${TMPDIR:-/tmp}/slopogram-player.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
ffmpeg -loglevel error -f lavfi -i 'color=c=blue:s=320x240:r=24:d=1' -an -c:v libx264 -pix_fmt yuv420p "$tmp/loop.mp4"
CLANG_MODULE_CACHE_PATH="$tmp/cache" swiftc -module-cache-path "$tmp/cache" \
  "$root/native/telegram-macos/packages/ChatUI/Sources/ChatUI/SlopogramWallpaperPlayer.swift" \
  "$root/native/telegram-patches/tests/WallpaperPlayerFixture.swift" -o "$tmp/fixture"
"$tmp/fixture" "$tmp/loop.mp4"
