#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ayu-proxy-test.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
printf 'import Foundation\n' > "$work_dir/Proxy.swift"
awk '/^public final class SlopogramProxyOverride/{copy=1} /^public enum ConnectionStatus/{copy=0} copy' native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/Network/Network.swift >> "$work_dir/Proxy.swift"
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swiftc -swift-version 5 "$work_dir/Proxy.swift" native/telegram-patches/tests/ProxyOverrideTest.swift -o "$work_dir/test"
"$work_dir/test"
