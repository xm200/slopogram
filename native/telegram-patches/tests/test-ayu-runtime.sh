#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ayu-runtime-test.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/ayu
sed '/^import Postbox$/d; /^import TelegramCore$/d; /^import TelegramUtils$/d; /^import TelegramContext$/d; /^import SwiftSignalKit$/d' native/telegram-macos/Telegram-Mac/account/AyuAccountRuntime.swift > "$work_dir/Runtime.swift"
module_dir=$(CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/ayu --show-bin-path)
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swiftc -swift-version 5 -I "$module_dir/Modules" -I native/ayu/Generated/FFI \
  native/telegram-patches/tests/AyuRuntimeStubs.swift \
  native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/Account/SlopogramGhostPolicy.swift \
  "$work_dir/Runtime.swift" native/telegram-patches/tests/AyuRuntimeLifecycleTest.swift \
  "$module_dir/SlopogramAyu.build/slopogram_ayu_ffi.swift.o" -L target/release -lslopogram_ayu_ffi \
  -Xlinker -rpath -Xlinker "$PWD/target/release" -o "$work_dir/test"
"$work_dir/test"
