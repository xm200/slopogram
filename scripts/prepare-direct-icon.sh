#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
source_icon="$PWD/assets/Slopogram.icon"
destination="$PWD/native/telegram-macos/Telegram-Mac/AppIcon.icon"
snapshot="$PWD/target/direct-icon-last-applied.icon"
[[ -f "$source_icon/icon.json" && -f "$source_icon/Assets/Plane.png" && -f "$source_icon/Assets/Knot.png" ]] || { echo 'Missing Slopogram Icon Composer document; run scripts/generate-macos-icon.sh.' >&2; exit 1; }
if diff -qr "$source_icon" "$destination" >/dev/null 2>&1; then
    if [[ ! -e "$snapshot" ]]; then mkdir -p target; cp -R "$source_icon" "$snapshot"; fi
    exit 0
fi
if [[ -n "$(git -C native/telegram-macos status --porcelain --untracked-files=all -- Telegram-Mac/AppIcon.icon)" ]] && ! diff -qr "$snapshot" "$destination" >/dev/null 2>&1; then
    echo 'The upstream icon has existing changes; preserve and resolve them before preparing the Slopogram icon.' >&2
    exit 1
fi
stage="$(mktemp -d "${TMPDIR:-/tmp}/slopogram-native-icon.XXXXXX")"
backup="$stage/AppIcon.original"
cleanup() {
    if [[ -d "$backup" && ! -e "$destination" ]]; then mv "$backup" "$destination"; fi
    rm -rf "$stage"
}
trap cleanup EXIT HUP INT TERM
cp -R "$source_icon" "$stage/AppIcon.icon"
mv "$destination" "$backup"
mv "$stage/AppIcon.icon" "$destination"
mkdir -p target
rm -rf "$snapshot"
cp -R "$source_icon" "$snapshot"
