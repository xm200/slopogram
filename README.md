# Slopogram

An experimental Telegram client inspired by AyuGram. The current macOS work is
a direct fork of Telegram macOS's original frontend using its upstream Rust
transport, with a separate Rust module for ghost controls and local edit/delete
history. The original Swift TelegramCore/Postbox account and model layers remain
for frontend compatibility. See [PLAN.md](PLAN.md) for migration status.

The current build targets Apple Silicon macOS only. The older application
bundles and standalone executables were removed at the user's request; their
source projects remain. The custom TGUIKit prototype below did not implement the
requested original Telegram screens and had a reported rendering failure.

## Direct macOS fork

Requires full Xcode with its Metal Toolchain component. Xcode's compiler must
work from the same desktop execution environment used for the build:

```sh
xcrun --sdk macosx metal --version
```

If the component is missing, install it with
`xcodebuild -downloadComponent MetalToolchain` before building. Component status
inside a restricted command sandbox can differ from the normal desktop session.

```sh
scripts/bundle-direct-upstream.sh
```

This builds the actual `Telegram-Mac.xcworkspace` target and its original screens,
with Slopogram patches and the standalone Rust Ayu library, for ARM64 only.
The first build
fetches pinned source dependencies and compiles the upstream native frameworks;
it takes substantially longer than the prototype build. Rust toolchains and
additional build tools are installed under `target`, without a global setup
installer. A successful bundle step produces
`target/release/Slopogram Native.app`.

To package that app for another Mac, run `scripts/package-native-dmg.sh`.
It creates `target/release/Slopogram Native.dmg` with the app and an
Applications shortcut. The script mounts the image read-only, checks the
signature, and compares the packaged app with the build output. The development
build uses an ad-hoc signature.

The macOS app icon uses the supplied black plane and gray knot. The editable
[Icon Composer document](assets/Slopogram.icon) contains the foreground artwork
and its glass settings. The [original logo](assets/slopogram-logo.png) is also
available for use on a white background. To regenerate the layer images and
legacy `.icns` after changing the artwork, run `scripts/generate-macos-icon.sh`
before bundling.

After signing in, choose **Slopogram → Import Theme…** and select the original
`.tdesktop-theme` file. Slopogram translates its colors, installs its embedded
wallpaper, and saves them together for the next launch. Tiled backgrounds repeat
across the chat. The converted Slopogram `.palette` files also load the matching
JPEG or PNG from the adjacent `Wallpapers` folder. A standalone palette or
color-only Desktop theme keeps your existing wallpaper. If automatic
appearance is enabled, the app asks whether to turn it off before applying the theme.

Use your own external `TELEGRAM_API_ID` and `TELEGRAM_API_HASH`; the fork does not
fall back to upstream's embedded credentials. It uses a separate Slopogram Native
application data directory. Existing grammers or installed Telegram sessions are
not migrated automatically. Source provenance and patch boundaries are in
[the direct fork notice](native/telegram-patches/UPSTREAM.md).

Deleted cloud text stays in the chat as a read-only bubble showing the original
text, a muted trash icon, and the original message time. Incoming messages stay
on the left and outgoing messages on the right. Capture is always enabled.
Only content observed by this client can be retained. Clearing local history
removes the saved copies and keeps capture enabled for future messages.

In the **Slopogram** menu, enable **Local Premium (only on this Mac)** for your
account. Your own name can show a star and a Unicode emoji status in Settings
and on your profile. Use **Set local emoji status…** and **Local profile color**
in the same menu to change the status and name color.

To use custom emoji in your own chat view, right-click one in the emoji picker
and choose **Use in my messages on this Mac**. Clicking that emoji in the chat
picker inserts the ordinary emoji into the composer. Slopogram displays the
custom artwork for that emoji in your messages on this Mac. Other clients
receive ordinary Unicode text. Choose **Restore default emoji on this Mac** to
remove the mapping. Turning Local Premium off hides the local appearance and
keeps your choices for next time.

The application menu beside the Apple menu contains each account's **Ghost
mode** switch. It suppresses automatic chat read receipts, future typing/activity
updates, and online presence updates. Explicit notification/mark-all read actions
are not covered; existing online status can remain until Telegram expires it.

**Turn on streamer mode** in that menu covers application windows with an opaque
privacy screen, uses generic titles and menus, and suppresses notification content
and badges. Turn it off from the privacy screen or the same menu. Its state is
saved per application profile. It also pauses automatic chat reads while the
screen is covered. This is an application privacy mask, not OS-wide screen-capture
protection; previously cached accessibility references are not proven safe.

The modes use event-driven updates. Ghost mode stops its online heartbeat timer
while suppressed; macOS Low Power Mode and accessibility settings feed the
original Lite Mode behavior. Retention uses batched SQLite writes. Battery-life
improvement has not been measured.

The direct fork is under implementation. See [PLAN.md](PLAN.md) for build and
manual acceptance status. Local history is plaintext and currently has no
automatic size limit. Media, secret-chat and thread/topic retention are not
implemented; channel range-clears that bypass individual deletion events are
also not captured.

## Historical TGUIKit prototype

Requires macOS 15+, a current Rust toolchain, and Xcode 26.1 or a compatible
Swift 6.2 toolchain. Build from the repository root:

```sh
scripts/bundle-telegram-frontend.sh
open "target/release/Slopogram macOS.app"
```

The setup script fetches pinned Telegram macOS UI sources and the SwiftSignalKit
dependency; the first build needs network access. It does not run upstream's
global setup installers. The resulting Apple Silicon bundle is ad-hoc signed
by default. Set `CODESIGN_IDENTITY` for another signing identity.

This frontend uses the actual upstream TGUIKit window, table and row framework,
with Slopogram Swift controllers adapting the existing Rust state. Telegram's
full `ChatController` and `ChatListController` require its TelegramCore/Postbox
backend; those controllers have not been transplanted. This version keeps
Rust/grammers as the sole Telegram engine. See
[source provenance and integration scope](native/telegramfrontend/UPSTREAM.md)
for the exact revisions, reused resources, and license notices.

QR and phone/code/2FA login, session restoration, conversations, text history,
send/receive, drafts, proxy configuration, and failed-send recovery use the
shared backend. The interface follows system light/dark appearance. Quit other
Slopogram frontends before opening it because they share an exclusive account
session. Configuration stays in the external `.env` or process environment
described below.

An offline fixture preview skips configuration and Telegram networking:

```sh
"target/release/Slopogram macOS.app/Contents/MacOS/SlopogramTelegram" --ui-preview
```

`--smoke-test` launches the fixture and exits automatically. Offline checks do
not establish live account behavior or rendered visual quality.

## Historical SwiftUI macOS frontend

Requires macOS 15+ and Xcode 26.1 or a compatible Swift 6.2 toolchain to build.
Native Liquid Glass is used on macOS 26+ with material fallback on macOS 15.

```sh
scripts/bundle-swift-macos.sh
```

Historically this produced `target/release/Slopogram Swift.app`, ad-hoc signed
by default, sharing the Iced frontend's account session. These generated apps
were removed at the user's request; only their source projects remain.

Configure the native app with the same external `.env` or process environment
described below. The native loader also checks for `.env` beside the app bundle,
and honors `SLOPOGRAM_CONFIG_FILE` and `SLOPOGRAM_DATA_DIR`. Explicit SOCKS proxy
configuration takes precedence over macOS system SOCKS settings. Credentials
and session files are never packaged in the app.

The native interface includes a reusable AppKit message list with incremental
updates and scroll anchoring, an NSTextView composer, system appearance,
focus-aware read receipts, and explicit recovery for unconfirmed sends. Return
sends, Shift-Return adds a newline, and input-method composition is preserved.

Offline preview is available without reading configuration or connecting to
Telegram:

```sh
"target/release/Slopogram Swift.app/Contents/MacOS/Slopogram" --ui-preview
```

Swift bindings are generated under ignored `native/macos/Generated` by the build
script. Regenerate them after changing the Rust FFI API.

## Run the Iced frontend

Install a current stable Rust toolchain. Development and local validation use Rust
1.99.1. The SQLite dependency compiles native code and requires a C/C++ toolchain
and libclang.

- macOS: Xcode Command Line Tools (`xcode-select --install`).
- Windows: Visual Studio C++ Build Tools, Windows SDK, and LLVM. Set
  `LIBCLANG_PATH` to LLVM's `bin` directory if bindgen cannot find libclang.
- Debian/Ubuntu: `build-essential pkg-config clang libclang-dev
  libfontconfig1-dev libxkbcommon-dev libwayland-dev libx11-dev libxi-dev
  libxrandr-dev libxcursor-dev`.

From the repository root:

```sh
cp .env.example .env
```

Obtain your application's own API ID and hash from
[my.telegram.org/apps](https://my.telegram.org/apps), then edit `.env`:

```dotenv
TELEGRAM_API_ID=your_numeric_application_id
TELEGRAM_API_HASH=your_32_character_application_hash
```

These are application credentials, not a bot token. Process environment variables
take precedence over files. Configuration is loaded from the first available
`.env` in the working directory, beside the executable, or in this build's source
repository. The repository fallback lets a locally built app bundle use your
existing development configuration when opened from Finder. For a relocated app,
set `SLOPOGRAM_CONFIG_FILE` to an external configuration file. Restart after
changing credentials. Credentials are never copied into the app bundle.

```sh
cargo run -p slopogram-desktop --locked
```

On a new session, scan the QR code from an already signed-in Telegram app:
Settings → Devices → Link Desktop Device. The code refreshes at expiry, and
Telegram's login update triggers completion. Two-step verification is requested
when enabled. Alternatively, choose **Use phone number** and enter your existing
account's international number and the code Telegram sends. You can switch back
with **Sign in with QR code**. Existing authorized sessions still skip login.

Selecting a chat fetches its
latest 100 messages. Read receipts are sent for the selected conversation while
the window has focus. Incoming messages appear automatically.
Press Enter or click Send to send plain text. Each chat keeps its own in-memory
draft.

An unconfirmed send keeps its text in a visible failed item. Check the conversation
before restoring and sending it again: a timeout can happen after Telegram has
accepted the original message. The app never automatically resends that message.

### Proxy

Set `SLOPOGRAM_PROXY=socks5://host:port` in `.env` to force a specific SOCKS5
proxy (username/password forms like `socks5://user:pass@host:port` also work).
When unset on macOS, Slopogram uses the system SOCKS proxy from System
Settings → Network → Proxies, if enabled. The Telegram transport supports
SOCKS5 only; a system HTTP proxy is ignored with a startup warning. A
malformed `SLOPOGRAM_PROXY` is a visible startup error, never silently
ignored.

To force a known proxy endpoint, set its SOCKS or mixed-protocol listener address
in `.env` (for example `SLOPOGRAM_PROXY=socks5://127.0.0.1:7890`, using your actual
listener port), then fully quit and reopen Slopogram. To follow macOS settings,
leave that variable unset and enable a SOCKS proxy in the current network service.
System HTTP/HTTPS settings and `HTTP_PROXY`/`HTTPS_PROXY` do not configure the
MTProto connection. Proxy changes take effect after restarting the app.

## Sessions

Grammers persists authorization keys, peer metadata, and synchronization state in
`telegram.sqlite` under the OS application data directory:

- macOS: `~/Library/Application Support/one.slopogram.Slopogram`
- Windows: `%LOCALAPPDATA%\slopogram\Slopogram\data`
- Linux: `$XDG_DATA_HOME/slopogram` (normally `~/.local/share/slopogram`)

Set `SLOPOGRAM_DATA_DIR` to override this with a dedicated private directory. Only
one process may use a session. On Unix, new directories have mode 700 and session
files have mode 600; existing shared directories are rejected. The OS releases
the session lock on exit, including after a crash.

**The prototype session database is not encrypted at rest.** It grants account
access: do not share it or commit it. Windows permissions follow the user's local
application directory. OS credential-store encryption is future work. Login codes
and 2FA passwords are not intentionally persisted. No message-content logging or
telemetry is configured.

On restart, the app checks saved authorization and skips login when it remains
valid. Live updates enable grammers catch-up. Synchronization state is saved as
updates are processed and during orderly shutdown. Recent history is fetched again
when a chat opens. There is no offline message database yet. Drafts and unconfirmed
sends do not persist across process restarts. Revoked/expired sessions require
login again. Corrupt databases are reported and preserved, never silently erased.

## Iced macOS integration

On macOS the app installs a native menu bar (About, Services, Hide/Show,
Quit ⌘Q, Window menu with Minimize ⌘M/Zoom/Bring All to Front) and a custom
dock icon through AppKit, in the `slopogram-macos` crate. Copy/cut/paste and
select-all already work in text fields through Iced. Menu actions route
through the standard responder chain.

## Appearance

Light and dark appearances follow the operating system by default and update
when the system appearance changes. The interface uses translucent panels,
rounded surfaces, restrained highlights, and theme-aware text and error colors.
QR graphics remain black on white in both appearances.

The native Swift frontend uses SwiftUI Liquid Glass for controls on macOS 26+
and native material fallback on macOS 15. Reduce Transparency uses opaque native
surfaces, and live message updates preserve their viewport without whole-list
animations.

The Iced macOS fallback uses a public AppKit `NSVisualEffectView` behind its
transparent window. Windows and Linux use adaptive Iced styling.

## Release binary

Build an optimized executable for your current platform:

```sh
cargo build -p slopogram-desktop --locked --release
```

The executable is `target/release/slopogram-desktop` (`.exe` on Windows).

Build a distributable `.app` bundle:

```sh
scripts/bundle-macos.sh
```

The bundle is ad-hoc signed by default (`target/release/Slopogram.app`). For
distribution set `CODESIGN_IDENTITY` to a Developer ID and notarize with
`xcrun notarytool` (requires the $99/year Apple Developer Program). The bundle
uses the flattened `assets/Slopogram.icns` app icon.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `slopogram-core` | Protocol-independent models, timeline merging, input validation |
| `slopogram-telegram` | Authentication, commands/updates, persistent sessions |
| `slopogram-client` | Managed runtime, ordered events, shared message reconciliation |
| `slopogram-ffi` | Typed UniFFI interface and Swift binding generator |
| `slopogram-desktop` | Iced fallback interface and interaction state |
| `slopogram-macos` | Native macOS menu bar, services menu, dock icon |
| `native/macos` | SwiftUI/AppKit frontend, configuration, native tests |
| `native/telegramfrontend` | TGUIKit presentation target over the shared Swift state |
| `native/telegram-macos` | Ignored, pinned upstream UI source checkout |

The interface communicates with a Tokio worker through bounded channels. Live
updates run independently of history and send requests. History snapshots, send
responses, and updates merge by conversation and message ID; deletion tombstones
stop an in-flight history request from resurrecting removed messages.

## Verification

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p slopogram-desktop --locked
scripts/build-swift-macos.sh
swift test --package-path native/macos
scripts/build-telegram-frontend.sh
swift test --package-path native/telegramfrontend
```

Tests are offline and never contact Telegram. CI defines Rust jobs for Windows,
macOS, and Linux and a native Swift build/test job on macOS; the presence of a
workflow does not mean those jobs have run.

Manual acceptance with your own account:

1. Start with a new dedicated session directory. Scan the QR from Telegram's
   Devices settings and, when enabled, complete 2FA. Leave a QR unscanned until
   expiry and confirm it refreshes. Switch to phone login and back to QR. Also
   check that incorrect phone codes/passwords show recoverable errors.
2. Open Saved Messages and confirm recent history appears.
3. Send a unique short message. Confirm it appears once here and in another client.
4. Send to Saved Messages from the other client; confirm it appears here without
   refreshing. Switch chats while history loads; verify messages stay in their chat.
5. Close and restart. Confirm login is skipped and history remains correct.
6. Close the app, send another message elsewhere, then restart and open that chat.
   Confirm the message appears after catch-up/history refresh.
7. Disconnect and restore networking. Check visible errors and recovery/reconnect
   without requesting another login code.
8. Open a second instance with the same session. It should report the session is
   already in use without changing the database.
9. Switch system appearance between light and dark, including while the app is
   running. Check login, chats, messages, inputs, and errors for readable contrast.
   On macOS, check the native backdrop and window resizing.

Live account checks require your credentials and are separate from offline tests.
See [PLAN.md](PLAN.md) for milestone status and [AGENTS.md](AGENTS.md) for contributor
instructions.

## Current limits

Single account, latest 100 messages per opened chat, plain-text composer. Media and
service messages use a placeholder. Older-history pagination, account registration,
notifications, calls, secret chats, and AyuGram-specific features are outside this
milestone. Minimum window size: 760 × 540 pixels; login scrolls to keep controls
available in smaller windows.
