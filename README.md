# Slopogram

Slopogram is an experimental Telegram client for Apple Silicon Macs, inspired
by AyuGram. It uses Telegram macOS’s original Swift frontend, TelegramCore and
Postbox, upstream Rust transport, and a Rust module for ghost controls and local
message history. See [PLAN.md](PLAN.md) for feature status.

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
cargo fetch --locked
scripts/bundle-direct-upstream.sh
```

This builds the actual `Telegram-Mac.xcworkspace` target and its original screens,
with Slopogram patches and the standalone Rust Ayu library, for ARM64 only.
The first build fetches pinned source dependencies and compiles the upstream
native frameworks. The build stores the pinned Rust toolchain and additional
tools under `target`. A successful bundle step produces
`target/release/Slopogram Native.app`.

To package that app for another Mac, run `scripts/package-native-dmg.sh`.
It creates `target/release/Slopogram Native.dmg` with the app, an
Applications shortcut, and a blank `Slopogram.env.example`. For a new Mac,
fill in your own API ID and hash and save the file at
`~/Library/Application Support/Slopogram Native/.env` before launching.
The installed app checks that location on startup. `SLOPOGRAM_CONFIG_FILE`
can select a different file, and process environment variables override
individual settings. The script mounts the image read-only, checks the
signature, and compares the packaged app with the build output. The development
build uses an ad-hoc signature.

The macOS app icon uses the supplied black plane and gray knot. The editable
[Icon Composer document](assets/Slopogram.icon) contains the foreground artwork
and its glass settings. The [original logo](assets/slopogram-logo.png) is also
available for use on a white background. To regenerate the layer images and
legacy `.icns` after changing the artwork, run `scripts/generate-macos-icon.sh`
before bundling.

After signing in, choose **Slopogram → Edit Theme…** to edit your current colors.
Use the color wells or hex fields, and search for specific interface elements.
**Choose Wallpaper…** accepts JPEG, PNG, or MP4. Selecting a video keeps your
color edits and generates a still wallpaper for Low Power Mode. The editor's
sample chat uses that still image while you edit.

Choose **Apply** to use the draft in Slopogram, or **Export…** to save its colors,
wallpaper, and optional video together as a `.tdesktop-theme` file. Exporting
does not change the active theme. Video playback uses Slopogram's MP4 extension;
the archive also contains the standard static wallpaper for Telegram Desktop.

Choose **Slopogram → Import Theme…** and select an MP4 to
generate a theme directly. Slopogram samples four frames for the interface
and bubble colors, uses the first video frame as the Low Power Mode wallpaper,
and saves both the loop and the still locally. The original file can then be moved.
Silent H.264 clips up to 30 seconds and 16 MiB keep their original quality
when each dimension is at most 1920 pixels and the frame rate is at most 60 fps.
Other supported MP4s up to 512 MiB use the first 15 seconds of video to make a
silent loop, or the whole clip if it is shorter.

To use an existing theme, select the original
`.tdesktop-theme` file. Slopogram translates its colors, installs its embedded
wallpaper, and saves them together for the next launch. Tiled backgrounds repeat
across the chat. The converted Slopogram `.palette` files also load the matching
JPEG or PNG from the adjacent `Wallpapers` folder. A standalone palette or
color-only Desktop theme keeps your existing wallpaper. If automatic
appearance is enabled, the app asks whether to turn it off before applying the theme.

For an animated chat wallpaper, put a silent H.264 clip at
`slopogram/background.mp4` inside the same theme ZIP as `colors.tdesktop-theme`
and a `background.jpg` or `background.png` poster. An eight-second 720p24 loop
and a matching first-frame poster work well. Slopogram accepts clips up to
16 MiB and 30 seconds, up to 60 fps, with each dimension at most 1920 pixels.
Import this file once. The active chat plays the loop, and macOS Low Power Mode,
Reduce Motion, or Slopogram’s animation or video power-saving setting shows the
poster. Leaving the chat or window also stops playback. Both assets are saved
locally, so you can move the original theme after importing.

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

## Proxy

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

## Source and verification

The supported app and release target is the direct Apple Silicon macOS fork.
The earlier Iced, SwiftUI, and TGUIKit prototypes remain in the repository for
reference; they are not supported builds or release targets. The active Rust
workspace contains the core models, Ayu policy and journal, Ayu FFI, and the
UniFFI binding generator. The upstream checkout is reproduced from pinned sources
and patches; see [source provenance](native/telegram-patches/UPSTREAM.md).

Run the workspace, Ayu, patch, and direct app checks documented in
[AGENTS.md](AGENTS.md). Live account and visual acceptance remain separate
from offline checks; never send real messages as an automated test.
