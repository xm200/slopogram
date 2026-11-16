#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
scratch="$(mktemp -d "${TMPDIR:-/tmp}/slopogram-streamer-fixture.XXXXXX")"
bundle="$scratch/SlopogramStreamerFixture.app"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$bundle/Contents/MacOS"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>org.slopogram.fixture.streamer</string><key>CFBundleExecutable</key><string>Fixture</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>
PLIST
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$scratch/clang}"
swiftc -swift-version 5 -emit-library -emit-module -module-name SwiftSignalKit \
 native/telegram-macos/submodules/telegram-ios/submodules/SSignalKit/SwiftSignalKit/Source/*.swift \
 -emit-module-path "$scratch/SwiftSignalKit.swiftmodule" -o "$scratch/libSwiftSignalKit.dylib"
swiftc -D DEBUG -swift-version 5 -I "$scratch" -L "$scratch" -lSwiftSignalKit \
 -Xlinker -rpath -Xlinker "$scratch" \
 native/telegram-macos/Telegram-Mac/account/SlopogramStreamerMode.swift \
 native/telegram-patches/tests/StreamerPrivacyFixture.swift -o "$bundle/Contents/MacOS/Fixture"
"$bundle/Contents/MacOS/Fixture" --saved-active
"$bundle/Contents/MacOS/Fixture"
