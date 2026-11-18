#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/setup-telegram-frontend.sh
UPSTREAM=native/telegram-macos
IOS="$UPSTREAM/submodules/telegram-ios"
if [[ "$(git -C "$UPSTREAM" config --bool core.sparseCheckout || true)" == true ]]; then
    git -C "$UPSTREAM" sparse-checkout disable
fi
git -C "$IOS" sparse-checkout add submodules third-party build-system/SwiftTL
git -C "$UPSTREAM" -c url.https://github.com/.insteadOf=git@github.com: -c url.https://gitlab.com/.insteadOf=git@gitlab.com: submodule update --init --depth 1 submodules/Sparkle submodules/tgcalls submodules/tg_owt submodules/tlottie submodules/libtgvoip submodules/libprisma submodules/regex third-party/twidget third-party/widget-sandbox
git -C "$UPSTREAM/third-party/widget-sandbox" submodule update --init --depth 1
git -C "$UPSTREAM/submodules/Sparkle" submodule update --init --depth 1 ed25519
git -C "$UPSTREAM/submodules/tg_owt" submodule update --init --depth 1
python3 scripts/prepare-direct-abseil.py
git -C "$IOS" -c url.https://github.com/.insteadOf=git@github.com: -c url.https://gitlab.com/.insteadOf=git@gitlab.com: submodule update --init --depth 1 third-party/libvpx/libvpx third-party/dav1d/dav1d third-party/td/td third-party/wallet-engine/wallet-engine third-party/mtproto-engine submodules/tlottie/tlottie
apply_patch_once() {
    local checkout="$1" patch="$2"
    if git -C "$checkout" apply --reverse --check "$patch" 2>/dev/null; then return; fi
    if ! git -C "$checkout" apply --check "$patch"; then
        printf 'Cannot apply %s without changing existing edits.\n' "$patch" >&2
        return 1
    fi
    git -C "$checkout" apply "$patch"
}
sh "$UPSTREAM/scripts/prepare-telegram-ios.sh"
PATCH_ROOT="$PWD/native/telegram-patches"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0001-slopogram-app.patch"
python3 scripts/apply-direct-upstream-config.py
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0002-ayu-app.patch"
# Upgrade the importer in checkouts prepared by earlier releases.
for upgrade in 0025-upgrade-theme-import.patch 0028-upgrade-direct-mp4-import.patch 0029-upgrade-theme-editor.patch; do
    if git -C "$UPSTREAM" apply --check "$PATCH_ROOT/$upgrade" 2>/dev/null; then
        git -C "$UPSTREAM" apply "$PATCH_ROOT/$upgrade"
    fi
done
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0018-streamer-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0020-theme-import-app.patch"
if [[ -f "$PATCH_ROOT/0002-ayu-telegram-ios.patch" ]]; then apply_patch_once "$IOS" "$PATCH_ROOT/0002-ayu-telegram-ios.patch"; fi
apply_patch_once "$IOS" "$PATCH_ROOT/0003-ghost-telegram-ios.patch"
apply_patch_once "$IOS" "$PATCH_ROOT/0004-postbox-journal-ios.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0005-updater-app.patch"
apply_patch_once "$IOS" "$PATCH_ROOT/0005-proxy-telegram-ios.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0006-arm64-build.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0007-xcode-compat-app.patch"
apply_patch_once "$IOS" "$PATCH_ROOT/0007-xcode-compat-ios.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0008-window-capture-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0009-weak-vars-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0010-gallery-capture-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0011-call-capture-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0013-chat-synthetic-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0015-debug-notification-skip-app.patch"
apply_patch_once "$IOS" "$PATCH_ROOT/0017-core-deletion-origin.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0016-ayu-projection-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0020-power-accessibility-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0022-local-premium-ui-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0023-local-emoji-chat-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0024-local-emoji-picker-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0026-live-theme-player-app.patch"
apply_patch_once "$UPSTREAM" "$PATCH_ROOT/0027-live-theme-chat-app.patch"
