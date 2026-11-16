#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../../.."
work_dir=$(mktemp -d "${TMPDIR:-/tmp}/ayu-notifications-test.XXXXXX")
trap 'rm -rf "$work_dir"' EXIT
python3 - "$work_dir/main.swift" <<'PY'
import pathlib, sys
source = pathlib.Path('native/telegram-macos/Telegram-Mac/utils/UNUserNotifications.swift').read_text()
start = source.index('    static var _current:UNUserNotifications?')
end = source.index('    static var current:UNUserNotifications?', start)
section = source[start:end]
text = '''import Foundation
final class SharedNotificationManager {}
class UNUserNotifications {
''' + section + '''
}
final class UNUserNotificationsNew: UNUserNotifications { init(manager: SharedNotificationManager) {} }
final class UNUserNotificationsOld: UNUserNotifications { init(manager: SharedNotificationManager) {} }
UNUserNotifications.initialize(manager: SharedNotificationManager())
print(UNUserNotifications._current == nil ? "skipped" : "initialized")
'''
pathlib.Path(sys.argv[1]).write_text(text)
PY
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swiftc -D DEBUG "$work_dir/main.swift" -o "$work_dir/debug"
CLANG_MODULE_CACHE_PATH="${CLANG_MODULE_CACHE_PATH:-$work_dir/clang}" swiftc "$work_dir/main.swift" -o "$work_dir/release"
test "$(SLOPOGRAM_SKIP_NOTIFICATIONS=1 "$work_dir/debug")" = skipped
test "$(SLOPOGRAM_SKIP_NOTIFICATIONS=0 "$work_dir/debug")" = initialized
test "$(SLOPOGRAM_SKIP_NOTIFICATIONS=1 "$work_dir/release")" = initialized
printf '%s\n' 'Notification initialization guard check passed'
