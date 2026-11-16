#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build -p slopogram-ffi --locked --release
mkdir -p native/macos/Generated/{Swift,FFI}
cargo run -p slopogram-ffi --bin uniffi-bindgen --locked --release -- generate --library target/release/libslopogram_ffi.dylib --language swift --out-dir native/macos/Generated
mv native/macos/Generated/*.swift native/macos/Generated/Swift/
mv native/macos/Generated/*.h native/macos/Generated/FFI/
mv native/macos/Generated/*.modulemap native/macos/Generated/FFI/module.modulemap
swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/macos -c release
