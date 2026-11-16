#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/setup-telegram-frontend.sh
if [[ ! -f native/telegram-macos/submodules/telegram-ios/submodules/SSignalKit/Package.swift ]]; then
    printf '%s\n' 'Missing pinned Telegram-iOS SSignalKit checkout; initialize the upstream dependency before building.' >&2
    exit 1
fi
scripts/build-swift-macos.sh
swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/telegramfrontend -c release
