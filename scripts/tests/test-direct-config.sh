#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TEST_DIR"' EXIT
SOURCE="$TEST_DIR/repo/native/telegram-macos/packages/ApiCredentials/Sources/ApiCredentials"
mkdir -p "$SOURCE" "$TEST_DIR/work" "$TEST_DIR/cache"
mkdir -m 700 "$TEST_DIR/data"
mkdir -m 755 "$TEST_DIR/shared"
ln -s "$TEST_DIR/data" "$TEST_DIR/link"
cp "$ROOT/native/telegram-patches/SlopogramExternalConfiguration.swift" "$SOURCE/"
printf 'TELEGRAM_API_ID=123\nTELEGRAM_API_HASH=fake\n' > "$TEST_DIR/repo/.env"
cat > "$TEST_DIR/Test.swift" <<'SWIFT'
import Foundation
@main struct Test {
    static func main() {
        switch CommandLine.arguments[1] {
        case "scope":
            let first = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
            let alias = URL(fileURLWithPath: CommandLine.arguments[3], isDirectory: true)
            let other = URL(fileURLWithPath: CommandLine.arguments[4], isDirectory: true)
            let a = SlopogramExternalConfiguration.profileKeychainScope(for: first)
            precondition(a == SlopogramExternalConfiguration.profileKeychainScope(for: first))
            precondition(a == SlopogramExternalConfiguration.profileKeychainScope(for: alias))
            precondition(a != SlopogramExternalConfiguration.profileKeychainScope(for: other))
            precondition(a.count == 82 && a.allSatisfy { $0.isASCII && ($0.isLowercase || $0.isNumber || $0 == "-") })
        case "fallback":
            precondition(SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.values["TELEGRAM_API_ID"] == "123")
        case "override":
            precondition(SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.values["TELEGRAM_API_ID"] == "456")
        case "missing":
            precondition(!SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.error != nil)
        case "proxy":
            precondition(SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.proxy?.host == "localhost")
            precondition(SlopogramExternalConfiguration.proxy?.port == 1080)
        case "baddata":
            precondition(!SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.error?.contains("private directory") == true)
        case "badproxy":
            precondition(!SlopogramExternalConfiguration.valid)
            precondition(SlopogramExternalConfiguration.error?.contains("SOCKS5") == true)
        default: preconditionFailure()
        }
    }
}
SWIFT
swiftc -module-cache-path "$TEST_DIR/cache" "$SOURCE/SlopogramExternalConfiguration.swift" "$TEST_DIR/Test.swift" -o "$TEST_DIR/test"
cd "$TEST_DIR/work"
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/data" "$TEST_DIR/test" fallback
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/data" TELEGRAM_API_ID=456 "$TEST_DIR/test" override
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/data" SLOPOGRAM_CONFIG_FILE="$TEST_DIR/missing" "$TEST_DIR/test" missing
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/data" SLOPOGRAM_PROXY=socks5://localhost:1080 "$TEST_DIR/test" proxy
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/data" SLOPOGRAM_PROXY=invalid "$TEST_DIR/test" badproxy
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/shared" "$TEST_DIR/test" baddata
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/link" "$TEST_DIR/test" baddata
env -i PATH=/usr/bin:/bin SLOPOGRAM_DATA_DIR="$TEST_DIR/new" "$TEST_DIR/test" fallback
[[ "$(stat -f %Lp "$TEST_DIR/new")" == 700 ]]

"$TEST_DIR/test" scope "$TEST_DIR/data" "$TEST_DIR/link" "$TEST_DIR/shared"
