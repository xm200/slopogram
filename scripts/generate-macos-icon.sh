#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ -f assets/slopogram-logo.png && -f assets/slopogram-foreground.png ]] || { echo 'Missing source logo or transparent foreground.' >&2; exit 1; }
stage="$(mktemp -d "${TMPDIR:-/tmp}/slopogram-icon.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/Slopogram.iconset" assets/Slopogram.icon/Assets
sips -s format png -z 1024 1024 assets/slopogram-logo.png --out "$stage/slopogram-icon.png" >/dev/null
for spec in '16 icon_16x16' '32 icon_16x16@2x' '32 icon_32x32' '64 icon_32x32@2x' '128 icon_128x128' '256 icon_128x128@2x' '256 icon_256x256' '512 icon_256x256@2x' '512 icon_512x512' '1024 icon_512x512@2x'; do
    read -r size name <<< "$spec"
    sips -z "$size" "$size" "$stage/slopogram-icon.png" --out "$stage/Slopogram.iconset/$name.png" >/dev/null
done
iconutil -c icns "$stage/Slopogram.iconset" -o "$stage/Slopogram.icns"
swift scripts/split-macos-icon.swift assets/slopogram-foreground.png "$stage/Plane.png" "$stage/Knot.png"
cp "$stage/slopogram-icon.png" assets/slopogram-icon.png
cp "$stage/Slopogram.icns" assets/Slopogram.icns
cp "$stage/Plane.png" assets/Slopogram.icon/Assets/Plane.png
cp "$stage/Knot.png" assets/Slopogram.icon/Assets/Knot.png
mkdir -p assets/slopogram.iconset
cp "$stage/Slopogram.iconset/"*.png assets/slopogram.iconset/
