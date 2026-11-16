#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build -p slopogram-ayu-ffi --locked --release --offline
install_name_tool -id @rpath/libslopogram_ayu_ffi.dylib target/release/libslopogram_ayu_ffi.dylib
cargo build -p slopogram-ffi --bin uniffi-bindgen --locked --release --offline
mkdir -p native/ayu/Generated/{Swift,FFI}
target/release/uniffi-bindgen generate --library target/release/libslopogram_ayu_ffi.dylib --language swift --out-dir native/ayu/Generated
mv native/ayu/Generated/*.swift native/ayu/Generated/Swift/
mv native/ayu/Generated/*.h native/ayu/Generated/FFI/
mv native/ayu/Generated/*.modulemap native/ayu/Generated/FFI/module.modulemap
swift build ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path native/ayu -c release
