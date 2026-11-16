#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ayu-signals-test.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
export CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}"
swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/ayu
module_dir=$(swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/ayu --show-bin-path)
swiftc -swift-version 5 -emit-module -emit-library -module-name SwiftSignalKit \
 native/telegram-macos/submodules/telegram-ios/submodules/SSignalKit/SwiftSignalKit/Source/*.swift \
 -emit-module-path "$work_dir/SwiftSignalKit.swiftmodule" -o "$work_dir/libSwiftSignalKit.dylib"
sed '/^import Postbox$/d; /^import TelegramCore$/d; /^import TelegramUtils$/d; /^import TelegramContext$/d' native/telegram-macos/Telegram-Mac/account/AyuAccountRuntime.swift > "$work_dir/Runtime.swift"
swiftc -swift-version 5 -I "$module_dir/Modules" -I native/ayu/Generated/FFI -I "$work_dir" \
 native/telegram-patches/tests/AyuRuntimeSignalStubs.swift \
 native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/Account/SlopogramGhostPolicy.swift \
 "$work_dir/Runtime.swift" native/telegram-patches/tests/AyuRuntimeSignalIntegrationTest.swift \
 "$module_dir/SlopogramAyu.build/slopogram_ayu_ffi.swift.o" -L target/release -lslopogram_ayu_ffi \
 -L "$work_dir" -lSwiftSignalKit -Xlinker -rpath -Xlinker "$work_dir" \
 -Xlinker -rpath -Xlinker "$PWD/target/release" -o "$work_dir/test"
"$work_dir/test"
