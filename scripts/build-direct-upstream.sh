#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
scripts/setup-direct-upstream.sh
scripts/prepare-direct-icon.sh
[[ -f native/telegram-patches/0002-ayu-app.patch ]] || { echo 'Missing Ayu app integration patch.' >&2; exit 1; }
scripts/build-ayu-macos.sh
if [[ ! -x target/direct-upstream-venv/bin/meson ]]; then
    python3 -m venv target/direct-upstream-venv
    target/direct-upstream-venv/bin/python -m pip install 'meson==1.9.2'
fi
export DEVELOPER_DIR="${DEVELOPER_DIR:-$(xcode-select -p)}"
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
YASM_ROOT="$PWD/target/direct-upstream-tools"
if [[ ! -x "$YASM_ROOT/yasm-install/bin/yasm" ]]; then
    mkdir -p "$YASM_ROOT"
    if [[ ! -d "$YASM_ROOT/yasm-src/.git" ]]; then
        git clone --depth 1 --branch v1.3.0 https://github.com/yasm/yasm.git "$YASM_ROOT/yasm-src"
    fi
    [[ "$(git -C "$YASM_ROOT/yasm-src" rev-parse HEAD)" == ba463d3c26c0ece2e797b8d6381b161633b5971a ]] || { echo 'Unexpected Yasm revision.' >&2; exit 1; }
    (cd "$YASM_ROOT/yasm-src" && ac_cv_header_stdc=yes ./autogen.sh --prefix="$YASM_ROOT/yasm-install" && make -j 4 && make install)
fi
if [[ -x /opt/homebrew/bin/cmake ]]; then PATH="/opt/homebrew/bin:$PATH"; fi
export PATH="$YASM_ROOT/yasm-install/bin:$PWD/target/direct-upstream-venv/bin:$PATH"
export RUSTUP_HOME="$PWD/target/direct-upstream-rustup"
export CARGO_HOME="$PWD/target/direct-upstream-cargo"
export CARGO="$(command -v rustup | xargs dirname)/cargo"
export CMAKE_POLICY_VERSION_MINIMUM=3.5
RUSTUP="$(command -v rustup)"
mkdir -p "$CARGO_HOME/bin"
if [[ ! -e "$CARGO_HOME/bin/rustup" ]]; then ln -s "$RUSTUP" "$CARGO_HOME/bin/rustup"; fi
if ! "$RUSTUP" run 1.96.1 rustc --version >/dev/null 2>&1; then
    "$RUSTUP" toolchain install 1.96.1 --profile minimal --target aarch64-apple-darwin
fi
for target in aarch64-apple-darwin; do "$RUSTUP" target add --toolchain 1.96.1 "$target"; done
bash native/telegram-macos/scripts/configure_frameworks.sh
xcodebuild -workspace native/telegram-macos/Telegram-Mac.xcworkspace -scheme Telegram -configuration Debug -derivedDataPath "$PWD/target/direct-upstream-derived" -destination 'generic/platform=macOS' CODE_SIGNING_ALLOWED=NO MACOSX_DEPLOYMENT_TARGET=15.0 ARCHS=arm64 ONLY_ACTIVE_ARCH=YES build
