#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fixture="$root/native/telegram-patches/tests/presence-fixture"
for module in SwiftSignalKit TelegramApi MtProtoKit Postbox; do
    swiftc -swift-version 5 -emit-library -emit-module -module-name "$module" -module-cache-path "$scratch/cache" "$fixture/$module.swift" -o "$scratch/lib$module.dylib" -emit-module-path "$scratch/$module.swiftmodule"
done
swiftc -swift-version 5 -warnings-as-errors -module-cache-path "$scratch/cache" -I "$scratch" -L "$scratch" -lSwiftSignalKit -lTelegramApi -lMtProtoKit -lPostbox -Xlinker -rpath -Xlinker "$scratch" "$root/native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/Account/SlopogramGhostPolicy.swift" "$root/native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/State/SlopogramOnlinePresenceLoop.swift" "$root/native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/State/ManagedAccountPresence.swift" "$fixture/Test.swift" -o "$scratch/test"
"$scratch/test"
