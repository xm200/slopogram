# Retired Telegram macOS presentation prototype

Slopogram's Telegram presentation target uses code and image assets from
[Telegram for macOS](https://github.com/TelegramMessenger/telegram-macos),
pinned to commit `246e3e957ae5cea1ecff6d7a6a6db3e47be69bb5`.
The upstream copyright and license notices remain in the source files. The
upstream GPL version 2 license is preserved in `LICENSE` beside this notice and
as `Telegram-LICENSE.txt` in the built application.

## Reused code and resources

- Upstream `packages/TGUIKit`, including its `Window`, `TableView`,
  `TableRowItem`, and `TableRowView` implementations.
- Its `ColorPalette`, `Colors`, `Strings`, `KeyboardKey`, and `ObjcUtils`
  dependencies.
- `SwiftSignalKit` from Telegram-iOS commit
  `24638bc69bd083981b178dd1e7f921eebbf7fc46`, under
  `submodules/SSignalKit`. Source:
  <https://github.com/TelegramMessenger/Telegram-iOS>.
- The `Icon_Quote`, `Icon_Quote_Collapse`, `Icon_Quote_Expand`,
  `Icon_SearchField`, and `Icon_SearchClear` image sets from the upstream
  `Telegram-Mac/Assets.xcassets`, compiled into the application's asset catalog.

The application controllers and record adapters in `Sources/SlopogramTelegram`
are Slopogram code. They connect these presentation primitives to the shared
`SlopogramState` Swift library and the Rust/grammers engine through UniFFI.

This target does not reuse the upstream account-bound `ChatController` or
`ChatListController`. Those require TelegramCore Account/TelegramEngine and
Postbox types. No TelegramCore/Postbox networking or second account session is
started. The current application covers Slopogram's existing single-account,
text-messaging scope; it does not implement the complete upstream feature set.

## Source provenance

This prototype is retained as reference source. The current application uses
the [direct Telegram macOS frontend](../telegram-patches/UPSTREAM.md).

The original upstream source can be inspected with:

```sh
git -C native/telegram-macos rev-parse HEAD
git -C native/telegram-macos/submodules/telegram-ios rev-parse HEAD
```

Preserve the applicable upstream licenses, copyright notices, and corresponding
source when distributing a derivative. This local bundle is ad-hoc signed by
default; it has not been notarized or published.
