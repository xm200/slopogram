#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-direct-upstream.sh
SOURCE="target/direct-upstream-derived/Build/Products/Debug/Telegram.app"
APP="${SLOPOGRAM_DIRECT_BUNDLE_PATH:-target/release/Slopogram Native.app}"
[[ -d "$SOURCE" ]] || { echo 'The upstream Telegram app did not build.' >&2; exit 1; }
mkdir -p target/release "$(dirname "$APP")"
STAGING="$(mktemp -d 'target/release/.Slopogram Native.staging.XXXXXX')"
STAGED="$STAGING/Slopogram Native.app"
BACKUP="target/release/.Slopogram Native.previous.$$"
cleanup() {
    if [[ -e "$BACKUP" && ! -e "$APP" ]]; then mv "$BACKUP" "$APP"; fi
    rm -rf "$STAGING"
}
trap cleanup EXIT HUP INT TERM
cp -R "$SOURCE" "$STAGED"
rm -rf "$STAGED/Contents/PlugIns" "$STAGED/Contents/XPCServices"
mkdir -p "$STAGED/Contents/Frameworks" "$STAGED/Contents/Resources"
cp target/release/libslopogram_ayu_ffi.dylib "$STAGED/Contents/Frameworks/"
cp native/telegram-macos/LICENSE "$STAGED/Contents/Resources/Telegram-LICENSE.txt"
cp native/telegram-patches/UPSTREAM.md "$STAGED/Contents/Resources/Telegram-UPSTREAM.md"
cp assets/Slopogram.icns "$STAGED/Contents/Resources/Slopogram.icns"
/usr/libexec/PlistBuddy -c 'Set :CFBundleName Slopogram' "$STAGED/Contents/Info.plist" || /usr/libexec/PlistBuddy -c 'Add :CFBundleName string Slopogram' "$STAGED/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :LSMinimumSystemVersion 15.0' "$STAGED/Contents/Info.plist" || /usr/libexec/PlistBuddy -c 'Add :LSMinimumSystemVersion string 15.0' "$STAGED/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleDisplayName Slopogram' "$STAGED/Contents/Info.plist" || /usr/libexec/PlistBuddy -c 'Add :CFBundleDisplayName string Slopogram' "$STAGED/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIconName AppIcon' "$STAGED/Contents/Info.plist" || /usr/libexec/PlistBuddy -c 'Add :CFBundleIconName string AppIcon' "$STAGED/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Set :CFBundleIconFile Slopogram.icns' "$STAGED/Contents/Info.plist" || /usr/libexec/PlistBuddy -c 'Add :CFBundleIconFile string Slopogram.icns' "$STAGED/Contents/Info.plist"
/usr/libexec/PlistBuddy -c 'Delete :SUFeedURL' "$STAGED/Contents/Info.plist" 2>/dev/null || true
/usr/libexec/PlistBuddy -c 'Set :SUEnableAutomaticChecks false' "$STAGED/Contents/Info.plist" 2>/dev/null || /usr/libexec/PlistBuddy -c 'Add :SUEnableAutomaticChecks bool false' "$STAGED/Contents/Info.plist"
codesign --force --sign "${CODESIGN_IDENTITY:--}" "$STAGED/Contents/Frameworks/libslopogram_ayu_ffi.dylib"
codesign --force --deep --sign "${CODESIGN_IDENTITY:--}" "$STAGED"
codesign --verify --deep --strict "$STAGED"
if [[ -e "$APP" ]]; then mv "$APP" "$BACKUP"; fi
if ! mv "$STAGED" "$APP"; then
    if [[ -e "$BACKUP" ]]; then mv "$BACKUP" "$APP"; fi
    exit 1
fi
rm -rf "$BACKUP"
printf 'Built %s\n' "$APP"
