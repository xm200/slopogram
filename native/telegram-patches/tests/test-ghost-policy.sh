#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"
git apply --include '*/SlopogramGhostPolicy.swift' "$root/native/telegram-patches/0003-ghost-telegram-ios.patch"
swiftc -module-cache-path "$scratch/module-cache" submodules/TelegramCore/Sources/Account/SlopogramGhostPolicy.swift "$root/native/telegram-patches/tests/GhostPolicyTest.swift" -o "$scratch/test"
"$scratch/test"
