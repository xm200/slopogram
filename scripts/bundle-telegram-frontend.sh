#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/build-telegram-frontend.sh
VERSION="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')"
APP="target/release/Slopogram macOS.app"
mkdir -p target/release
STAGING="$(mktemp -d 'target/release/.Slopogram macOS.staging.XXXXXX')"
STAGED_APP="$STAGING/Slopogram macOS.app"
BACKUP="target/release/.Slopogram macOS.previous.$$"
cleanup() {
    if [[ -e "$BACKUP" && ! -e "$APP" ]]; then mv "$BACKUP" "$APP"; fi
    rm -rf "$STAGING"
}
trap cleanup EXIT HUP INT TERM
mkdir -p "$STAGED_APP/Contents/MacOS" "$STAGED_APP/Contents/Resources"
cp native/telegramfrontend/.build/release/SlopogramTelegram "$STAGED_APP/Contents/MacOS/SlopogramTelegram"
cp assets/Slopogram.icns "$STAGED_APP/Contents/Resources/Slopogram.icns"
cp native/telegramfrontend/LICENSE "$STAGED_APP/Contents/Resources/Telegram-LICENSE.txt"
cp native/telegramfrontend/UPSTREAM.md "$STAGED_APP/Contents/Resources/Telegram-UPSTREAM.md"
CATALOG="$STAGING/TelegramAssets.xcassets"
mkdir -p "$CATALOG"
printf '%s\n' '{"info":{"author":"xcode","version":1}}' > "$CATALOG/Contents.json"
for asset in Icon_Quote Icon_Quote_Collapse Icon_Quote_Expand Icon_SearchField Icon_SearchClear; do
    cp -R "native/telegram-macos/Telegram-Mac/Assets.xcassets/$asset.imageset" "$CATALOG/"
done
xcrun actool "$CATALOG" --compile "$STAGED_APP/Contents/Resources" --platform macosx --minimum-deployment-target 15.0
cat > "$STAGED_APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Slopogram</string>
<key>CFBundleIdentifier</key><string>one.slopogram.Slopogram</string>
<key>CFBundleExecutable</key><string>SlopogramTelegram</string>
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
