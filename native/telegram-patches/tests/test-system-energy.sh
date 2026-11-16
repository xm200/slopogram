#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cd "$scratch"
git apply --include '*/SlopogramSystemEnergy.swift' "$root/native/telegram-patches/0020-power-accessibility-app.patch"
printf 'public enum LiteModeKey: String { case any, emoji_effects, emoji, blur, dynamic_background, gif, video, stickers, animations, menu_animations, liquidGlass }\n' > "$scratch/InAppSettings.swift"
swiftc -emit-library -emit-module -module-name InAppSettings -module-cache-path "$scratch/cache" "$scratch/InAppSettings.swift" -o "$scratch/libInAppSettings.dylib" -emit-module-path "$scratch/InAppSettings.swiftmodule"
swiftc -module-cache-path "$scratch/cache" -I "$scratch" -L "$scratch" -lInAppSettings -Xlinker -rpath -Xlinker "$scratch" "$scratch/Telegram-Mac/account/SlopogramSystemEnergy.swift" "$root/native/telegram-patches/tests/SystemEnergyTest.swift" -o "$scratch/test"
"$scratch/test"
