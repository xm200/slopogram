#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
UPSTREAM=native/telegram-macos
UPSTREAM_REV=246e3e957ae5cea1ecff6d7a6a6db3e47be69bb5
IOS_REV=24638bc69bd083981b178dd1e7f921eebbf7fc46
if [[ ! -e "$UPSTREAM" ]]; then
    git clone --depth 1 --filter=blob:none --no-checkout https://github.com/TelegramMessenger/telegram-macos.git "$UPSTREAM"
    git -C "$UPSTREAM" fetch --depth 1 origin "$UPSTREAM_REV"
    git -C "$UPSTREAM" sparse-checkout init --cone
    git -C "$UPSTREAM" sparse-checkout set packages/TGUIKit packages/ColorPalette packages/Colors packages/Strings packages/KeyboardKey packages/ObjcUtils Telegram-Mac/Assets.xcassets/Icon_Quote.imageset Telegram-Mac/Assets.xcassets/Icon_Quote_Collapse.imageset Telegram-Mac/Assets.xcassets/Icon_Quote_Expand.imageset Telegram-Mac/Assets.xcassets/Icon_SearchField.imageset Telegram-Mac/Assets.xcassets/Icon_SearchClear.imageset
    git -C "$UPSTREAM" checkout --detach "$UPSTREAM_REV"
fi
if [[ "$(git -C "$UPSTREAM" rev-parse HEAD)" != "$UPSTREAM_REV" ]]; then
    printf '%s\n' 'Telegram macOS checkout has an unexpected revision; preserve it and resolve the mismatch manually.' >&2
    exit 1
fi
IOS="$UPSTREAM/submodules/telegram-ios"
if [[ ! -e "$IOS/.git" ]]; then
    git clone --depth 1 --filter=blob:none --no-checkout https://github.com/TelegramMessenger/Telegram-iOS.git "$IOS"
    git -C "$IOS" fetch --depth 1 origin "$IOS_REV"
    git -C "$IOS" sparse-checkout init --cone
    git -C "$IOS" sparse-checkout set submodules/SSignalKit
    git -C "$IOS" checkout --detach "$IOS_REV"
fi
if [[ "$(git -C "$IOS" rev-parse HEAD)" != "$IOS_REV" || ! -f "$IOS/submodules/SSignalKit/Package.swift" ]]; then
    printf '%s\n' 'Telegram-iOS checkout is incomplete or has an unexpected revision; resolve it manually.' >&2
    exit 1
fi
