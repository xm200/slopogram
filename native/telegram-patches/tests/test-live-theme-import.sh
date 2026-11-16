#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
ffmpeg -v error -f lavfi -i color=c=red:s=64x64:r=24 -t 1 -an -c:v libx264 -pix_fmt yuv420p "$tmp/valid.mp4"
python3 - "$root/native/telegram-macos/Telegram-Mac/app/AppDelegate.swift" "$tmp/Test.swift" <<'PY'
import sys
s = open(sys.argv[1]).read()
start = s.index('    nonisolated private static func slopogramStoreVideoBytes')
end = s.index('\n    nonisolated private static func slopogramDiscardPreparedWallpaper', start)
function = s[start:end].replace('nonisolated private static func', 'static func', 1)
open(sys.argv[2], 'w').write('''import Foundation
import AVFoundation
import CoreMedia
class Fixture {
    enum SlopogramThemeImportError: Error { case invalidWallpaper }
''' + function + '''
}
let root = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
let data = try Data(contentsOf: root.appendingPathComponent("valid.mp4"))
let cache = root.appendingPathComponent("cache", isDirectory: true)
if CommandLine.arguments[2] == "write" {
try Fixture.slopogramStoreVideoBytes(data, destination: cache.appendingPathComponent("poster-a.mp4"))
let saved = cache.appendingPathComponent("poster-a.mp4")
precondition((try! Data(contentsOf: saved)) == data)
precondition(!FileManager.default.fileExists(atPath: cache.appendingPathComponent("poster-b.mp4").path))
do {
    try Fixture.slopogramStoreVideoBytes(Data("invalid".utf8), destination: cache.appendingPathComponent("poster-b.mp4"))
    fatalError("invalid video accepted")
} catch {}
precondition(!FileManager.default.fileExists(atPath: cache.appendingPathComponent("poster-b.mp4").path))
precondition((try! Data(contentsOf: saved)) == data)
} else {
    let saved = cache.appendingPathComponent("poster-a.mp4")
    precondition((try! Data(contentsOf: saved)) == data)
    precondition(!FileManager.default.fileExists(atPath: cache.appendingPathComponent("poster-b.mp4").path))
}
print("video import fixture passed: \(CommandLine.arguments[2])")
''')
PY
CLANG_MODULE_CACHE_PATH="$tmp/clang-cache" swiftc -module-cache-path "$tmp/swift-cache" "$tmp/Test.swift" -o "$tmp/test"
"$tmp/test" "$tmp" write
"$tmp/test" "$tmp" read
