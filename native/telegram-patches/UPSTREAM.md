# Direct Telegram macOS fork

This integration uses the original application and frontend from
[Telegram macOS](https://github.com/TelegramMessenger/telegram-macos), pinned to
`246e3e957ae5cea1ecff6d7a6a6db3e47be69bb5`. It builds the
`Telegram-Mac.xcworkspace` application target, including the upstream auth,
chat list, chat, input, navigation, themes, localization, and resources.

The Telegram-iOS dependency is pinned to
`24638bc69bd083981b178dd1e7f921eebbf7fc46`. The original Swift TelegramCore,
Postbox, account, and model layers remain because the original frontend uses
them directly. Its `MTProtoRustEngine` adapter selects the upstream Rust
transport. The user explicitly selected this architecture on 2026-11-16.
The Rust transport source is
[TelegramMessenger/mtproto-engine](https://github.com/TelegramMessenger/mtproto-engine)
at `da4b2170210aab0430c4ff18c032991f3030cf45`.
The separate Slopogram grammers engine is retained for fallback clients and is
not started alongside the direct application's network engine.

## Slopogram changes

Source changes are reproduced by `scripts/setup-direct-upstream.sh` from the
patches and overlays in this directory. Changes include application identity,
external application-credential loading, isolated application data, and the
standalone Rust `slopogram-ayu`/`slopogram-ayu-ffi` feature library and its
`SlopogramAyu` Swift bindings. The exact implemented feature coverage and checks
are recorded in the root `PLAN.md`.

The existing custom-screen `native/telegramfrontend` target is an earlier
TGUIKit prototype. It is a separate application and does not establish acceptance
of the direct frontend migration.

## Source and license preservation

The upstream GPL version 2 license is retained at
`native/telegram-macos/LICENSE` and copied into a built direct application as
`Telegram-LICENSE.txt`. Preserve upstream copyright notices and the applicable
third-party licenses with source and derivative distributions. The checkout also
contains the licenses of its pinned dependencies.

The scripts do not publish an application or upload an archive. A local bundle
is ad-hoc signed by default, with no notarization. Build scripts never bundle
the external `.env` file or account/session data. API credentials must be supplied
for Slopogram; the fork must not fall back to upstream's embedded credentials.

## Reproduction

```sh
scripts/setup-direct-upstream.sh
scripts/bundle-direct-upstream.sh
```

The first build fetches pinned dependencies and compiles the upstream native
frameworks. Read `PLAN.md` for current build prerequisites and unresolved checks;
the presence of scripts or patches does not mean a working app has been built.
