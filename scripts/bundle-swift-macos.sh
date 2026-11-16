#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-swift-macos.sh
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
APP="target/release/Slopogram Swift.app"
mkdir -p target/release
STAGING="$(mktemp -d 'target/release/.Slopogram Swift.staging.XXXXXX')"
STAGED_APP="$STAGING/Slopogram Swift.app"
BACKUP="target/release/.Slopogram Swift.previous.$$"
cleanup() {
    if [[ -e "$BACKUP" && ! -e "$APP" ]]; then mv "$BACKUP" "$APP"; fi
    rm -rf "$STAGING"
}
trap cleanup EXIT HUP INT TERM
mkdir -p "$STAGED_APP/Contents/MacOS" "$STAGED_APP/Contents/Resources"
cp native/macos/.build/release/Slopogram "$STAGED_APP/Contents/MacOS/Slopogram"
cp assets/Slopogram.icns "$STAGED_APP/Contents/Resources/Slopogram.icns"
cat > "$STAGED_APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Slopogram</string>
<key>CFBundleIdentifier</key><string>one.slopogram.Slopogram</string>
<key>CFBundleExecutable</key><string>Slopogram</string>
<key>CFBundleIconFile</key><string>Slopogram.icns</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundleVersion</key><string>$VERSION</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
codesign --force --deep --sign "${CODESIGN_IDENTITY:--}" "$STAGED_APP"
codesign --verify --deep --strict "$STAGED_APP"
if [[ -e "$APP" ]]; then mv "$APP" "$BACKUP"; fi
if ! mv "$STAGED_APP" "$APP"; then
    if [[ -e "$BACKUP" ]]; then mv "$BACKUP" "$APP"; fi
    exit 1
fi
rm -rf "$BACKUP"
echo "Built $APP"
