# Repository guidance

## Product and scope

Slopogram is an experimental Rust desktop Telegram client inspired by AyuGram.
Read `PLAN.md` for the current milestone and its acceptance criteria. The only supported app, build, and release is the Apple Silicon native
Telegram macOS fork. The goal is the actual
Telegram macOS frontend with its upstream Rust transport and a Rust Ayu feature
module, as explicitly selected by the user on 2026-11-16. Ghost controls and
local deleted/edited history are now authorized feature work.
Deleted-text retention is always enabled and rendered inline; do not expose an
enable/view switch. Clearing retained history must not disable future capture.
Streamer mode masks all personal information and message content; Ghost mode
suppresses outgoing activity. Use event-driven native power/accessibility
observers, batched storage, and no new idle polling for these modes.
Preserve historical prototype source projects and account data; do not build or
release the historical applications.

## Architecture

- `crates/core`: backend-independent models and message state. No Iced or
  grammers types here.
- `tools/uniffi-bindgen` generates the Swift bindings for the Ayu library.
- `crates/ayu` and `crates/ayu-ffi`: backend-independent Rust policies and an
  always-on observed-message journal, with standalone UniFFI bindings in `native/ayu`.
  Journal keys are account-scoped and include conversation/message identity.
  Global deletions require explicit namespace eligibility from the caller;
  never infer peer type from numeric IDs across different backends.
- The old desktop, macos, telegram, client and ffi crate directories and the
  Swift sources under `native/macos` and `native/telegramfrontend` are archived
  implementation references outside the active build.
- `native/telegram-macos`: ignored pinned dependency checkout, reproduced by
  `scripts/setup-telegram-frontend.sh`. Preserve upstream notices and do not run
  upstream's global setup or shipping scripts as part of local builds.
- The direct fork keeps upstream Swift TelegramCore/Postbox and its existing
  Rust network transport; do not introduce a second grammers account/session.
  Keep its application identity and data isolated from installed Telegram and
  the grammers prototype. Save all upstream changes as reproducible patches
  outside the ignored source checkout.
- Network and session I/O run asynchronously, outside GUI rendering/update code.
- Do not implement MTProto cryptography or reconnect logic from scratch.

## Working conventions

- Use stable Rust, workspace dependencies, and the committed Cargo.lock.
- Keep Telegram-specific types inside the Telegram crate.
- Identify messages by conversation and message ID; merge concurrent history,
  update, and send results without duplicates or losing newer state.
- Keep errors visible and recoverable. Never silently reset a corrupt session.
- Never log or commit phone numbers, login codes, 2FA passwords, API hashes,
  authorization keys, `.env`, or session databases and their sidecar files.
- Do not derive debug output for secret-bearing backend commands.
- Treat QR login URLs and migration tokens as credentials: never log them or
  expose them through Debug. Refresh QR tokens on login updates or expiration,
  and keep cancellation responsive while a QR request is pending.
- Follow system light/dark appearance by default. Use theme-derived styles;
  preserve black-on-white QR contrast regardless of application appearance.
- Keep `.env` external to release binaries and app bundles.
- Swift presentation state belongs on the main actor. Keep message version and
  deletion reconciliation in Rust, preserve stable scroll anchors, and never
  resend unconfirmed messages automatically.
- Bridge shutdown must wake event consumers and release runtime/session resources
  before reconnecting. Use explicit lifecycle control rather than relying on
  Swift task cancellation crossing the FFI boundary.
- Never send real Telegram messages as a test without an explicit user request.
- Keep this document concise; update PLAN.md when milestone status changes.

## Validation

Run these before handing off implementation changes:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For the current direct frontend, also run the Ayu/patch checks and ARM64 build:

```sh
scripts/build-ayu-macos.sh
swift test --package-path native/ayu
scripts/tests/test-direct-config.sh
native/telegram-patches/tests/test-ghost-policy.sh
native/telegram-patches/tests/test-ayu-runtime.sh
native/telegram-patches/tests/test-proxy-override.sh
native/telegram-patches/tests/test-postbox-journal.sh
native/telegram-patches/tests/test-ayu-runtime-signals.sh
native/telegram-patches/tests/test-ayu-projection.sh
native/telegram-patches/tests/test-deletion-origin.sh
native/telegram-patches/tests/test-presence-manager.sh
native/telegram-patches/tests/test-system-energy.sh
python3 scripts/tests/test-direct-patch-series.py --compare-working-tree
scripts/bundle-direct-upstream.sh
```

When a desktop execution environment is available, run
`native/telegram-patches/tests/test-streamer-privacy-gui.sh`. It creates only
synthetic AppKit windows and must not load a Telegram account. A passing fixture
does not prove every upstream window path or cached accessibility reference is
protected, nor does it measure battery life.

Skip historical macOS Iced/SwiftUI/prototype app builds and bundles. They would
recreate applications the user requested removing.

The native `--smoke-test` mode uses local fixtures and must not read credentials
or contact Telegram. A process smoke test does not establish visual quality or
live account functionality. In a nested sandbox, the optional
`SLOPOGRAM_SWIFT_BUILD_FLAGS=--disable-sandbox` and a writable
`CLANG_MODULE_CACHE_PATH` may be needed; native AppKit startup may require a normal
desktop execution environment.

Test meaningful state transitions, race handling, validation, and persistence.
Do not require Telegram credentials or a running display server in unit tests.
Record live account and platform checks separately; do not claim they passed if
they were not performed.
