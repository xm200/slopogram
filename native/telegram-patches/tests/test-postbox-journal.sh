#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
swift test ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path "$root/native/telegram-patches/tests/postbox-journal"
