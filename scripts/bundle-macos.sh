#!/usr/bin/env bash
# Build a distributable Slopogram.app for macOS.
# Icon: run `scripts/generate-macos-icon.sh` after updating the source artwork.
# Signing: ad-hoc by default. For distribution set CODESIGN_IDENTITY to your
# Developer ID and run notarization separately (`xcrun notarytool`).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(cargo metadata --no-deps --format-version 1 \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
APP="target/release/Slopogram.app"
mkdir -p target/release
STAGING="$(mktemp -d "target/release/.Slopogram.app.staging.XXXXXX")"
STAGED_APP="$STAGING/Slopogram.app"
BACKUP="target/release/.Slopogram.app.previous.$$"
cleanup() {
    if [[ -e "$BACKUP" && ! -e "$APP" ]]; then
        mv "$BACKUP" "$APP"
    fi
    rm -rf "$STAGING"
}
trap cleanup EXIT HUP INT TERM

cargo build -p slopogram-desktop --locked --release

mkdir -p "$STAGED_APP/Contents/MacOS" "$STAGED_APP/Contents/Resources"
cp target/release/slopogram-desktop "$STAGED_APP/Contents/MacOS/slopogram-desktop"
cp assets/Slopogram.icns "$STAGED_APP/Contents/Resources/Slopogram.icns"

cat > "$STAGED_APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Slopogram</string>
  <key>CFBundleDisplayName</key><string>Slopogram</string>
  <key>CFBundleIdentifier</key><string>one.slopogram.Slopogram</string>
  <key>CFBundleExecutable</key><string>slopogram-desktop</string>
  <key>CFBundleIconFile</key><string>Slopogram.icns</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.social-networking</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>NSHumanReadableCopyright</key><string>Slopogram prototype. Not affiliated with Telegram.</string>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

codesign --force --deep --sign "${CODESIGN_IDENTITY:--}" "$STAGED_APP"
codesign --verify --deep --strict "$STAGED_APP"

# Only replace the current bundle after the staged app is fully signed and verified.
if [[ -e "$APP" ]]; then
    mv "$APP" "$BACKUP"
fi
if ! mv "$STAGED_APP" "$APP"; then
    if [[ -e "$BACKUP" ]]; then mv "$BACKUP" "$APP"; fi
    exit 1
fi
rm -rf "$BACKUP"
echo "Built $APP (signed with ${CODESIGN_IDENTITY:-ad-hoc identity})"
