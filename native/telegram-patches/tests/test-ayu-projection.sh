#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ayu-projection-test.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
sed '/^import TelegramContext$/d' native/telegram-macos/packages/ChatUI/Sources/ChatUI/AyuDeletedProjection.swift > "$work_dir/Projection.swift"
python3 - "$work_dir" <<'PY'
import pathlib,sys
out=pathlib.Path(sys.argv[1])
a=pathlib.Path('native/telegram-macos/packages/TGUIKit/Sources/System.swift').read_text()
b=pathlib.Path('native/telegram-macos/packages/TelegramUtils/Sources/TelegramUtils/System.swift').read_text()
(out/'Reverse.swift').write_text('import Foundation\n'+a[a.index('public func reverseIndexList<T>'):a.index('public func delay(')])
(out/'Transitions.swift').write_text('import Foundation\n'+b[b.index('public func proccessEntriesWithoutReverse'):b.index('public func DALDevices()')])
PY
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swiftc -swift-version 5 \
 native/telegram-macos/packages/TGUIKit/Sources/MergeLists.swift "$work_dir/Reverse.swift" "$work_dir/Transitions.swift" \
 "$work_dir/Projection.swift" native/telegram-patches/tests/AyuProjectionTransitionTest.swift -o "$work_dir/test"
"$work_dir/test"
