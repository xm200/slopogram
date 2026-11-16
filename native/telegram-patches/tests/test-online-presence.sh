#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"
git apply --include '*/SlopogramOnlinePresenceLoop.swift' "$root/native/telegram-patches/0003-ghost-telegram-ios.patch"
swiftc -module-cache-path "$scratch/cache" submodules/TelegramCore/Sources/State/SlopogramOnlinePresenceLoop.swift "$root/native/telegram-patches/tests/OnlinePresenceLoopTest.swift" -o "$scratch/test"
"$scratch/test"
