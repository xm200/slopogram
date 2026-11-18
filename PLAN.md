# Slopogram milestone record

The only supported app, build, and release target is the direct Telegram macOS
fork for Apple Silicon. Earlier Iced, SwiftUI, TGUIKit, Windows, and Linux
work and validation below are historical; their sources remain for reference.
Historical artifacts and commands do not describe current supported releases.

## Native theme editor

The native macOS menu opens **Edit Theme…** with the current palette and locally
rendered image wallpaper. Color wells and hex fields edit a private draft with
a sample chat preview. The editor accepts JPEG, PNG, and MP4 wallpapers. Videos
use the existing silent-loop preparation and first-frame poster for Low Power
Mode. The editor preview uses the poster to avoid continuous playback.

**Apply** uses the existing theme import transaction and automatic-appearance
confirmation. **Export…** writes a `.tdesktop-theme` ZIP atomically with Desktop
color definitions, a JPEG/PNG wallpaper, and the optional
`slopogram/background.mp4` extension. Colors use the importer's existing mapping
between the Desktop and native palettes. Shared Desktop keys take the first
native mapping when multiple native colors differ. Exporting and closing the
editor leave the active theme settings intact.

Acceptance checks include color and alpha round-trips, invalid colors and
archives, PNG/JPEG/MP4 preparation, the synthetic editor window, reproducible
patch replay, and the Apple Silicon app and DMG build. Live account rendering
and opening the export in official Telegram Desktop are recorded separately.

On 2026-11-18, workspace formatting, tests and strict Clippy passed. All 11 Swift
tests passed with the synthetic editor fixtures enabled, including color-well
synchronization, invalid drafts across search, and applying an exported archive.
The native policy, wallpaper import, low-power player and patch replay checks
passed. The Apple Silicon app built and the mounted DMG matched its signed app
with eight ARM64 Mach-O files and a blank external configuration template.
The view-cache image omits some layer-rendered text, and Computer Use access to
the standalone preview app was denied. Full desktop visual verification and
live-account application remain pending the user's test.

## Session memory and journal maintenance

The archived grammers session keeps the latest 500 message IDs per chat and looks up individual
messages by ID. The update worker reports its initial connection state and later
transitions. Deletion tracking has separate 500-ID limits for scoped and
account-wide deletions. Each history request records when it started so an
overlapping deletion still takes precedence if its individual ID was evicted.

The Ayu journal replaces its event log with a transactionally written snapshot
above 4,096 rows. Archived messages and their versions remain in that snapshot.
Each Ayu feature instance queues storage calls on one worker and releases its
journal lock when dropped.

## Historical: Linux desktop packaging

The former Arch VCS recipe packaged the Linux Iced client. That recipe,
its GitHub Actions workflow and its downloadable package have been retired.

## Direct MP4 theme import

The macOS **Import Theme…** panel accepts a local MP4 alongside the existing
theme formats. It derives the interface and bubble colors from sampled frames,
saves a JPEG of the first frame, and sends those assets through the existing
theme transaction and wallpaper player. Compatible silent H.264 video is reused.
Other supported video is trimmed and converted to a short silent loop. Native
Low Power Mode and Reduce Motion continue to use the stored poster.

The distributable DMG includes a blank configuration template. The installed
app reads the user's private settings from Application Support or an explicit
`SLOPOGRAM_CONFIG_FILE` override.

An isolated import of the 720p aurora clip verified palette parsing, a matching
1280 × 720 JPEG, byte-for-byte video preservation, and restoration after removing
the source. A portrait 120 fps fixture exercised conversion and reimport. The
AppKit player fixture exercised low-power fallback and resume. The mounted arm64
DMG matched the signed app and passed the blank-template and account-data checks.

## Animated chat wallpapers — Apple Silicon test build

**Authoring format.** Ship the colors, a still image of the first frame, and an optional silent H.264 MP4 together. The user selects one file in **Slopogram → Import Theme…**. An illustrated theme uses this layout

```text
Maomao.tdesktop-theme
├── colors.tdesktop-theme
├── background.jpg           (PNG also supported)
└── slopogram/background.mp4
```

The video entry is a Slopogram extension. Telegram Desktop looks up the standard palette and JPEG/PNG entries, so its local importer can use the still image. Keep the **whole archive under 5 MiB** for sharing with Telegram Desktop. Start with a seamless 8-second loop at 720p, 24 fps, 8-bit SDR 4:2:0, around 1.5 Mb/s. A 1080p24 version around 3 Mb/s is an option when it fits alongside the poster. [Telegram Desktop theme loader](https://github.com/telegramdesktop/tdesktop/blob/dev/Telegram/SourceFiles/window/themes/window_theme.cpp)

**Compatibility check.** A 1.65 MB synthetic 720p clip was added to temporary copies of all six existing AyuGram themes. The released-format Rust importer read all six palettes and returned their original wallpaper bytes. Maomao was 2.37 MB packed. Rei had only 409 bytes to spare under 5 MiB, so check the final ZIP size for each theme.

**Resource baseline on Apple M4.** Each synthetic clip played through the hardware decoder for three real-time loops. All 576 frames decoded. The 720p clip used 1.65 MB on disk, 0.31 seconds of process CPU over 24 seconds, and 3.69 MB of additional sampled process memory after decoder setup. The 1080p clip used 3.28 MB, 0.27 CPU seconds, and 4.87 MB on the same measures. These short runs measure decoding only. The full player, macOS compositor, and battery impact need an in-chat measurement.

1. **Import and restore.** The Rust reader bounds the optional entry to 16 MiB. The native importer validates the local H.264 track, rejects audio and external media references, and decodes the first frame. It installs the video under the local poster's resource ID before committing the theme settings. An isolated AccountManager/Postbox test applied Maomao's colors, poster, and MP4, reopened them in another process after moving the source, and checked static and invalid-video fallback.
2. **Play in the chat.** The active conversation resolves the clip from its effective wallpaper, including per-chat overrides. An `AVPlayerLayer` above the poster becomes visible on the first decoded frame. A synthetic AppKit window exercised the actual player through a full loop and chat disappearance. [Apple’s looping player API](https://developer.apple.com/documentation/avfoundation/avplayerlooper)
3. **Respect motion and power settings.** Power and visibility notifications tear down playback under Low Power Mode, Reduce Motion, animation-saving Lite Mode, Streamer Mode, or an inactive chat/window. The window fixture simulated a low-power change, verified the player layer was removed while the poster remained, then restored playback.
4. **Before public release.** Measure scrolling, dropped frames, memory, and energy against the same still theme in a signed-in conversation. Check a per-chat override and an actual system Low Power Mode switch there. Test the sample's static fallback in official Telegram Desktop.

For a later palette-driven effect, reuse the native animated gradient and derive its stops directly from the theme colors. A gradient that moves briefly when a chat opens needs no video asset and can stay idle between interactions.

## Current work: direct Telegram macOS fork with Rust Ayu features

Corrected on 2026-11-16 after the user rejected the custom TGUIKit frontend and
reported its broken rendering. The previous implementation reused toolkit
primitives rather than the requested original screens. It is an experimental
prototype, not acceptance of the direct frontend migration.

The user explicitly selected **the upstream Rust transport with the original
screens**, overriding the earlier requirement to keep grammers on macOS. They
selected **ghost controls plus local deleted/edited history** as the first Ayu
features. Root plans, integrates, and reviews; coding is delegated to GPT-6.1
Terra at medium effort.

Latest build scope, clarified on 2026-11-16: **Apple Silicon macOS only; stop
Intel compilation.** Remove the older generated Slopogram application bundles.
Keep source projects and account/configuration data; these are not application
bundle cleanup targets. Do not rebuild fallback applications during this work.

Cleanup completed: removed `Slopogram.app`, `Slopogram Swift.app`, and
`Slopogram macOS.app` from `target/release`, plus their standalone release/debug
executables. No Slopogram bundles were found in `/Applications` or
`~/Applications`. No old Slopogram instance was running when normal quit was
requested. Account data and source projects were preserved.

### Architecture and preserved code

The Apple Silicon build now includes a native Liquid Glass app icon using the
supplied plane and knot. Icon Composer previews were checked in default, dark,
clear and tinted appearances. **Slopogram → Import Theme…** accepts Telegram
Desktop `.tdesktop-theme` files and native macOS `.palette` files. The Rust
decoder maps Desktop colors and reads the embedded background. The native app
prepares its wallpaper caches before saving the colors and wallpaper together.
A fresh process opened the same isolated profile and rendered all six supplied
wallpapers from disk, including the active theme read from the account manager.
Additional checks covered tiled images, color-only import, invalid image data,
and a failed cache write.
Converted Desktop palettes also resolve a matching JPEG or PNG from the adjacent
`Wallpapers` directory. All six original/converted pairs produced the same
wallpaper bytes in the isolated import and restart fixture.

Offscreen AppKit checks rendered the retained text, trash icon, and original
time with the actual chat row in light and dark themes, including incoming,
outgoing, and wrapped text. The live two-account deletion check is still open.

**Local Premium** stores its switch, profile-name color, Unicode status, and
custom emoji mappings in the account's local Ayu journal. The native account
and self-profile headers observe the setting. Users can map a custom emoji
from the chat picker and insert the ordinary Unicode character. Only the
display copy of self-authored chat text receives custom artwork. Telegram
continues to own the subscription, profile, and message entities.
Offline checks exercised account-isolated persistence, menu toggling and color
selection, independent removal of emoji mappings, and the native text layout
for composed Unicode emoji in a message. Live account rendering remains a
manual check.

Use the real `Telegram-Mac.xcworkspace` application and its existing
`AuthController`, `MainViewController`, `ChatListController`, `ChatController`,
row views, composer, theme system, localization, and resources. Keep Swift
TelegramCore/Postbox because these controllers require their account and model
types. Networking uses upstream `MTProtoRustEngine` through
`NetworkEngineFactory`, already selected by the pinned application's launch
path. Do not silently run the separate grammers session alongside it.

The grammers/Iced and earlier Swift frontends are archived as source. Their
session databases cannot be treated as interchangeable with the direct fork's
Postbox/account data. Use separate Slopogram identity and application data for
the direct app; do not touch an installed Telegram account or its files.

### Delegated implementation

1. `swift_build`: initialize pinned workspace and native dependencies, generate
   API/serialization sources, build native frameworks and the actual application,
   and create reproducible setup/build/signing scripts. Isolate branding,
   application identity, external API credentials, updater and telemetry settings.
   Keep all source modifications reproducible outside the ignored checkout.
2. `swift_bridge`: implement a reusable Rust policy and durable local history
   module, with a Swift-consumable API and offline tests. Ghost mode starts off;
   observed-message capture is always enabled.
   Store only observed versions; key records by account, conversation and message.
   Report storage failures. Preserve confirmation and deletion reconciliation.
3. `swift_ui`: connect the Rust module through the real upstream account/message
   paths, add Ayu settings/history using upstream controller patterns, and retain
   the original auth/chat/list screens. Preserve source patches reproducibly.
4. Root: review privacy/session boundaries, test the feature library and patch
   application, verify that the actual app target builds, and distinguish source
   work from live account and rendered UI acceptance.

### Feature boundaries

- Ghost controls suppress selected outgoing presence/typing/read operations at
  their actual emission points. Describe each implemented control precisely;
  this is not a guarantee that Telegram cannot infer account activity.
- Local history always records observed cloud text versions and deletion events.
  It cannot recover unseen or previously deleted content. Deleted text appears
  in its original position as a read-only bubble, with a muted trash icon beside
  the original time and alignment that follows the message direction.
  Keep retained messages local and separate from Postbox and server state.
- Provide explicit history clearing, private storage, and no content/token/
  credential logging. Clearing does not disable future capture. Never send
  account messages during tests.

### Acceptance

- [x] Build the actual upstream app/screens with its Rust transport.
- [x] Use Slopogram identity/data paths and external application credentials.
- [x] Ghost controls consult the Rust policy at real upstream emission points.
- [x] Rust local history persists edits/deletions and can be queried/cleared.
- [x] Initial application menu exposed Ghost controls and local history.
- [x] Replace the initial archive controls with always-on inline retention.
- [x] Integrate Streamer mode and event-driven energy-saving behavior.
- [x] Preserve reproducible source patches and pinned dependency provenance.
- [x] Offline tests and signing/build checks pass for the initial produced artifact.
- [x] User confirmed the original QR screen renders and QR authentication works.
- [ ] Exercise live deletion, Ghost/Streamer, messaging, and restart flows.

### Remote-deletion follow-up

Latest user requirements supersede opt-in and viewer controls: retained deleted
text is **always enabled and shown inline automatically**, with no enable/view
buttons. Streamer mode must mask all personal information and message content;
Ghost mode is a separate activity-suppression mode. Both modes cooperate with
macOS energy-saving behavior. These changes are now incorporated in the rebuilt
local application; live feature acceptance remains separate below.

Current additions:

- Force capture active for new and migrated policies, remove enable/view controls,
  and keep capture active after clearing retained history.
- Replace repeated whole-JSON rewrites with batched SQLite WAL storage and safe
  migration. Queries are scoped to one conversation. Full startup replay and
  unbounded retention remain explicit storage limitations.
- Stop Ghost-mode online heartbeat timers while suppressed; react to policy
  changes to resume, rather than polling for changes.
- Respect macOS Low Power Mode and reduced-motion/transparency settings through
  event-driven native observers and original Lite Mode behavior.
- Install a profile-scoped Streamer privacy mask before application windows are
  displayed, with generic titles/menus and suppressed badges/notifications.
  Use an opaque native curtain without continuous animation or polling. Do not
  claim OS-wide capture exclusion or a no-frame-leak guarantee without evidence.

The first direct build was produced at `target/release/Slopogram Native.app`:
ARM64-only executable, Debug application dylib and Rust Ayu dylib; approximately
409 MB; ad-hoc signature and plist checks passed. The user confirmed live QR
authentication works and then closed the application. The earlier animated QR
placeholder belonged to an isolated network-denied startup fixture, not normal
login. That fixture was closed and its temporary profile removed.

The first build kept an opt-in text archive. The current build retains observed
cloud text directly in chat when another participant deletes it.

Current follow-up work:

- Rust records explicit remote deletion origin and sticky local-hide intent.
  A server echo must not restore a message intentionally deleted in this client.
  Legacy archive entries with unknown origin stay out of the chat projection.
- The chat timeline gives each retained bubble a stable identity. Its trash icon
  and timestamp identify the deletion. Server actions apply only to live messages.
- Historical candidate: the opt-in setting was relabeled **Keep deleted messages
  locally and save edit history**. This switch is now superseded by the explicit
  always-enabled requirement above and must be removed.
- Candidate builds are staged separately while the user may be running the
  first build. Do not overwrite a running application before an orderly update.
- Automated tests cover deletion origin, local/remote arrival permutations,
  restart, exact/global collision isolation and projected timeline transitions.
  The follow-up build is complete; the manual remote-deletion check remains.

Updated artifact installed on 2026-11-16 at
`target/release/Slopogram Native.app` (approximately 410 MB). The actual upstream
Xcode target builds successfully, with all seven embedded Mach-O files verified
ARM64-only. Signature, plist, identity, updater and bundle-data checks pass. The
final Swift bridge round-trip and real SwiftSignalKit-to-Rust deletion/capture-
after-clear fixture pass against the rebuilt library. The closed previous app
bundle was replaced with rollback protection and then removed; account data was
not changed. Only `Slopogram Native.app` remains in `target/release`.

This remains an ad-hoc signed Debug development build, not a notarized release.
It was left closed. No real account messages were sent or inspected during
verification. To accept live anti-delete behavior, open this build, observe a new
text message, delete it from the other account, and confirm its bubble and trash
icon remain across reopening the chat and restarting the app. Streamer/Ghost
behavior with real windows/notifications and battery measurements remain manual
acceptance items; prior QR authentication confirmation applies to the first build.

Follow-up checks completed before the final application build:

- 57 Rust workspace tests, formatting and strict Clippy pass. After a narrow
  redundant-scan cleanup, the 18 Ayu/FFI tests and affected-crate Clippy pass.
- Always-on runtime lifecycle, real SwiftSignalKit-to-Rust retention and original
  chat projection transition fixtures pass, including capture after clearing.
- The production online-presence manager fixture covers policy changes, stopped
  timers, per-account isolation, resume and stale callbacks. Power/accessibility
  policy fixtures pass.
- Desktop AppKit fixtures pass for saved Streamer state, newly opened windows,
  replacement content, title/subtitle/toolbar changes, guarded menu mutation,
  repeated toggles, and normal menu/Dock state restoration. These fixtures use
  synthetic content and no Telegram account or network client.
- All 20 source patches apply and reverse-check from the pinned revisions. The
  patched working files byte-match that clean replay, including the per-profile
  keychain and updater changes. The configuration overlay has separate fixtures.
- Streamer masks the normal accessibility tree. Direct queries through previously
  retained accessibility references are not proven safe, and no OS screen-capture
  history purge or universal frame-leak guarantee is claimed.
- Retention uses one SQLite transaction per observation array and skips unchanged
  records. A delayed coalescer was not introduced: its failure/quit ordering was
  not sufficiently established. There is no new idle polling or intentional
  buffering delay, and battery-life improvements have not been measured.

Remaining broader Ayu gaps include full Settings integration, media retention,
secret-chat retention, thread/topic in-chat projection, channel range/bulk clears
that bypass individual deletion IDs (`applyUpdateMinAvailableMessage`), full read-receipt coverage,
archive encryption, retention limits, searching/export and richer per-message
edit-history UI. No complete AyuGram parity is claimed.

### Initial artifact verification (before the current follow-up)

- 48 Rust workspace tests, formatting, and strict Clippy pass. The standalone
  Rust Ayu dylib and Swift bindings build independently from grammers.
- The initial policies started disabled. Journal tests covered opt-in, account/chat identity,
  explicit global-ID eligibility, edit/deletion arrival permutations, sticky
  deletion state, persistence, private files, corruption, clearing and ordering.
  The journal retains distinct late observed versions without restoring anything
  to the ordinary timeline.
- The real Postbox package builds with the committed-message hook. Its offline
  integration test verifies old/new snapshots for two edits in one transaction,
  delivery after commit, a read of the committed final text, and disposal. The
  minimal fixture emits two peer-notification-table SQL messages; an equivalent
  test with no journal subscriber reproduces them. This fixture behavior remains
  unresolved and is not attributed to the hook.
- The real Rust Swift bridge passes a round-trip test. Runtime lifecycle tests
  use real bindings with lightweight account stubs and cover policy loading,
  account isolation, subscription lifetime, clear-and-disable, and queued-write
  flushing on quit. These do not establish real controller type correctness.
- Source patches connect automatic peer/thread read suppression, future
  typing/activity and online updates, opted-in committed text observations,
  deletion notifications, a local-history viewer and clearing. Read controls do
  not suppress explicit mark-all/notification read actions. Already submitted
  activity/online updates cannot be retracted, and existing online status may
  remain until expiry.
- Configuration fixture tests cover external credentials, precedence, private
  `SLOPOGRAM_DATA_DIR`, and explicit/system SOCKS. No actual credentials or account
  data were read during tests.
- All native frameworks have built. The interrupted Intel build was stopped;
  its completed ARM WebRTC objects were reused and checked. Reproducible build
  patches now target ARM64 only, including the host code generator and Rust
  targets. Cached completed universal libraries may be reused for their ARM slice.
- The initial direct Xcode application build and signing succeeded. A
  user-provided screenshot confirmed the original QR login screen renders, and
  the user confirmed normal QR authentication works. Computer Use inspection
  was unavailable because its permissions were not granted. Other live feature
  checks remain separate from this authentication confirmation.
- After the user authorized installation and resumed work, the Metal component
  import reported it was already installed. A normal desktop check confirms
  component `17B54` is installed and `xcrun --sdk macosx metal --version` runs
  successfully. The earlier restricted-sandbox status was misleading. The
  actual workspace build now passes Metal compilation; remaining missing
  pinned source dependencies are being initialized as Xcode discovers them.

The follow-up replaces JSON rewrites with SQLite WAL transactions and migrates
existing journals. Startup still replays the full event log into memory; there
is no automatic pruning or at-rest encryption. Unseen or previously deleted
content cannot be recovered.

## Historical prototype: Telegram macOS toolkit integration

Requested on 2026-11-15. The user explicitly selected keeping Slopogram's
Rust/grammers engine and adapting the frontend from
https://github.com/TelegramMessenger/telegram-macos. Root coordinates; the three
GPT-6.1 Terra medium coding agents continue implementation.

### Source and integration boundary

Upstream is checked out at `native/telegram-macos`, pinned to
`246e3e957ae5cea1ecff6d7a6a6db3e47be69bb5`. Its Telegram-iOS dependency is pinned to
`24638bc69bd083981b178dd1e7f921eebbf7fc46`; only the SwiftSignalKit subtree needed
by TGUIKit is checked out.

The existing upstream ChatUI/ChatListUI controllers require concrete
TelegramCore Account/TelegramEngine and Postbox models. They do not provide a
backend-independent interface. This integration therefore uses the real
upstream TGUIKit presentation framework and adapted Swift screen controllers
over the Rust state. It does not instantiate a second Telegram account or
silently switch to TelegramCore networking. Existing Slopogram features define
the functional scope; importing upstream's full media/calls/admin feature set
would require additional backend and presentation work.

### Delegated work

1. `swift_bridge`: extract the current Swift Store into public SlopogramState,
   shared by the current SwiftUI app and the new Telegram presentation target.
   Preserve Rust event ordering, reconciliation, sessions, proxy configuration,
   authentication, send recovery, and shutdown.
2. `swift_ui`: implement the new `native/telegramfrontend` controllers using
   upstream TGUIKit TableView/row/control APIs and source-derived presentation.
   Carry over login, chat list, history, composer, incremental updates, native
   appearance, focus handling, and explicit failed-send recovery.
3. `swift_build`: isolate upstream UI dependencies in a separate Swift package,
   pin source checkouts, provide repeatable build/bundle scripts and source
   attribution, and preserve both current working frontends.
4. Root: verify source boundaries and provenance, run integrated checks, review
   unsupported upstream assumptions, build the native artifact, and report the
   exact scope of reuse and actual validation.

### Acceptance

- [x] Build imports actual upstream TGUIKit code at the pinned revision.
- [x] A shared Swift state library connects both native frontends to Rust.
- [x] Current QR/phone/2FA, session restoration, chat/history/send/receive paths
      are wired through the adapter.
- [x] No TelegramCore/Postbox account or upstream API credentials are introduced.
- [x] Source/license notices and dependency setup are reproducible.
- [x] Rust and Swift tests, builds, and offline native smoke checks pass.
- [x] A signed Telegram-presentation Slopogram app is built alongside fallbacks.
- [ ] Live Telegram behavior and rendered appearance are checked when available.

### Implementation record

Implemented through the three GPT-6.1 Terra medium coding agents, with root
coordination, review, and integrated verification. The new executable lives in
the separate `native/telegramfrontend` Swift package. `Store` is now a shared
`SlopogramState` library; reconnect waits for the previous engine's shutdown
before starting another one. Existing Rust reconciliation and session handling
remain authoritative.

The frontend uses upstream TGUIKit's real `Window`, `TableView`, `TableRowItem`,
and `TableRowView`, with custom Swift controllers and record adapters. It
includes system light/dark palettes, a native material sidebar with reduced
transparency support, chat search and selection, multiline selectable messages,
stable viewport anchoring, bottom-follow for incoming messages, IME-safe
composition, native edit shortcuts, key-window read receipts, QR and phone
authentication, and explicit failed-send recovery. QR rendering uses a white
quiet border and integer scaling.

The setup script pins both source repositories, checks revisions, and fetches
only the required UI packages, SwiftSignalKit, and image assets. Build and bundle
scripts generate the Rust bridge and include the upstream GPL license and source
notice. Package dependency inspection confirms the graph contains TGUIKit,
ColorPalette, Colors, Strings, KeyboardKey, ObjcUtils, SSignalKit (SwiftSignalKit), and
Slopogram's libraries, with no TelegramCore/Postbox package.

### Integration verification on 2026-11-15

- 39 Rust workspace tests pass; rustfmt, strict Clippy, and the Iced build pass.
- 9 shared native state/configuration tests and 3 new frontend tests pass. The
  new tests cover row insertion/deletion/edit identity, selection updates, and
  following incoming messages only when initially opening or already at bottom.
- Both native release bundles rebuild successfully. The new artifact is
  `target/release/Slopogram macOS.app`, an approximately 23 MB Apple Silicon
  bundle requiring macOS 15+. It is ad-hoc signed; signature and plist checks
  pass. Dynamic-library inspection shows system dependencies, with no separate
  Rust, TGUIKit, or SwiftSignalKit library required at runtime.
- Three consecutive launches of the final new bundle's `--smoke-test` exit
  zero outside the command sandbox. Fixtures assert two conversation rows and
  two history rows after layout, include incoming/outgoing multiline text and
  same-number message IDs in different chats, and do not load credentials or
  start the backend. The rebuilt SwiftUI fallback also passes an offline launch.
- Required upstream image assets are compiled into `Assets.car`. Bundled
  license and source notices match the repository copies. Both pinned upstream
  checkouts retain their expected revisions; upstream source has no local edits.
- Existing Iced and SwiftUI app bundles remain available. Windows/Linux CI,
  live QR/phone/2FA/send/receive/restart acceptance, and rendered UI inspection
  were not performed. Process smoke checks do not establish visual quality.

Reproduce the new native checks with:

```sh
scripts/bundle-telegram-frontend.sh
swift test --package-path native/macos
swift test --package-path native/telegramfrontend
"target/release/Slopogram macOS.app/Contents/MacOS/SlopogramTelegram" --smoke-test
```

In this managed sandbox, builds used a writable `/tmp` Clang module cache,
`SLOPOGRAM_SWIFT_BUILD_FLAGS=--disable-sandbox`, and offline Cargo dependencies.
Desktop startup checks required execution outside the command sandbox.

Earlier work below is historical.

## Historical: native Swift macOS frontend

Requested on 2026-11-15. Root acts as orchestrator and reviewer; implementation
is delegated to GPT-6.1 Terra at medium reasoning effort. Earlier work below is
historical. The Rust Telegram implementation remains shared; Iced remains
available on Windows/Linux and as a migration fallback on macOS.

### Delegated implementation

1. `swift_bridge`: introduce a frontend-independent Rust client engine and a
   typed UniFFI bridge with a managed Tokio runtime, ordered events, explicit
   shutdown, existing authentication/proxy/session reuse, and shared message
   reconciliation. Register new workspace crates without changing grammers.
2. `swift_ui`: build native SwiftUI navigation/login/sidebar/settings and AppKit
   message-list/text-composer components. Preserve QR/phone/code/2FA login,
   sending/receiving, drafts, send recovery, focus-aware read receipts, and
   session restoration. Follow system appearance and support reduced motion.
3. `swift_build`: generate Swift bindings, link the Rust static library, compile
   a Swift package, resolve external configuration and system SOCKS proxy, and
   stage/sign/verify a native macOS app. Never embed credentials or sessions.
4. Root: review the shared contract and state ownership, resolve integration
   problems through agents, run Rust/Swift checks, build the release artifact,
   and record the difference between offline checks and live acceptance.

### Native design and platform scope

Target macOS 15+ with native Liquid Glass on macOS 26+ and material fallback on
older supported versions. Use the platform font, a compact material sidebar,
quiet conversation content, and restrained blue accents. Prefer reusable AppKit
message rows and an NSTextView composer; preserve scroll position during updates
and avoid animating whole history batches. Keep QR codes black on white and
provide clear loading, error, connection, and empty states.

### Native acceptance

- [x] Rust engine/bridge builds, owns its runtime, and exposes typed Swift APIs.
- [x] Existing session/config/proxy paths are reused without migration or reset;
      actual account reuse remains part of live acceptance.
- [x] Native app has QR, phone/code, and optional 2FA login wiring.
- [x] Chat list/history and sending/receiving are wired to the real Rust backend.
- [x] Native reusable message list/composer implement incremental row changes,
      stable scroll anchoring, text selection, and input-method composition.
- [x] System light/dark appearance, native glass/fallback, and accessibility
      preferences are honored.
- [x] Offline Rust and Swift tests and native smoke mode pass without credentials.
- [x] Existing Rust/Iced checks and the local fallback build pass; Windows/Linux
      implementation is preserved and remote CI remains unrun.
- [x] Signed native Swift release bundle is compiled and verified.
- [ ] Live account behavior and rendered UI are checked when access is available.

### Native implementation and verification record

Implemented by the three delegated GPT-6.1 Terra medium agents and reviewed and
integrated by root. The native artifact is
`target/release/Slopogram Swift.app` (Apple Silicon ARM64, approximately 14 MB
executable), with the Rust library statically linked. `otool -L` confirms no
runtime dependency on `libslopogram_ffi.dylib`; app signature, plist, and declared
minimum OS (15.0) were verified. The original Iced app remains available.

Minimum OS was adjusted to 15 because the installed Rust standard library targets
15. The native Rust build sets the same deployment target for its C/SQLite code.

Final checks on 2026-11-15:

- 39 Rust tests pass across the complete workspace.
- Strict workspace Clippy, rustfmt, and the Iced debug build pass.
- 8 Swift tests pass for configuration, proxy precedence, authoritative snapshots,
  deletion handling, actionable errors, and explicit failed-send recovery.
- Rust lifecycle tests verify shutdown wakes a pending consumer, releases held
  resources before returning, rejects later sends, and tolerates repeated shutdown.
- A deleted-before-confirmation race now emits a confirmation without restoring
  deleted content, so the frontend can clear its pending-send state.
- Native release build, ad-hoc signature verification, and plist checks pass.
- Three consecutive standalone `--smoke-test` launches exit zero outside the
  command sandbox, using fixtures without configuration or network access.

The first smoke attempts aborted in the command sandbox or hung when quitting
inside the initial SwiftUI task. Delegating termination to the next main-queue
turn and immediately permitting termination when no engine exists fixed the
standalone smoke checks. Normal termination awaits bounded backend shutdown.

Commands used (sandbox-specific environment flags are optional outside this
managed environment):

```sh
cargo fmt --all --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build -p slopogram-desktop --locked --offline
CLANG_MODULE_CACHE_PATH=/tmp/slopogram-native-clang-cache \
  SLOPOGRAM_SWIFT_BUILD_FLAGS=--disable-sandbox \
  CARGO_NET_OFFLINE=true scripts/bundle-swift-macos.sh
CLANG_MODULE_CACHE_PATH=/tmp/slopogram-native-clang-cache \
  swift test --disable-sandbox --package-path native/macos
```

Live Telegram login/send/receive/session-reuse and rendered appearance have not
been verified. No live Telegram messages were sent. New native CI is defined but
has not run remotely. Native process smoke checks are not visual QA.

## Historical: release binary, glass appearance, QR login

Requested on 2026-11-15. The root agent coordinates implementation and review;
coding tasks are delegated to GPT-6 Luna with medium reasoning effort.

### Work split

1. Telegram agent: add QR token export/import, expiration and refresh, migrated
   datacenter handling, two-step verification, and cancellation to phone login.
   Keep login tokens out of logs and Debug output; reuse persistent sessions.
2. Desktop agent: replace fixed light colors with semantic dark/light tokens,
   follow OS appearance by default, and build glass-like layered surfaces with
   readable text and controls. Add scannable QR login and phone fallback.
3. Native/build agent: add a supported macOS translucent material backdrop and
   preserve existing menus, icon, and proxy behavior. Stage, sign, and verify the
   release app before replacing an existing bundle. Never bundle credentials.
4. Root: review interfaces and cancellation behavior, integrate dependency
   changes, run workspace checks, compile the optimized native binary, and build
   the macOS app bundle. Record actual verification and artifact paths.

### Design decisions

Use platform sans-serif type and the established two-pane messenger layout.
Light mode uses frosted white and cool blue-gray surfaces; dark mode uses layered
slate surfaces with soft white highlights. Rounded translucent chrome, restrained
blue accents, and subtle shadows create the glass treatment. QR stays black on
white in either appearance for reliable scanning. Prefer public AppKit material
APIs on macOS, with a styled Iced fallback on Windows/Linux. This does not assume
Iced can reproduce Apple's complete native Liquid Glass rendering.

### Extension acceptance

- [x] System appearance is delegated to Iced's native light/dark selection.
- [x] Theme-aware glass styling covers login, conversations, and failure states;
      macOS adds a public AppKit material behind the original Iced content view.
- [x] QR login generates scannable tokens, refreshes expired tokens, supports
      datacenter migration and 2FA, and cancels cleanly to phone login.
- [x] QR token Debug output is redacted and transition/encoding tests pass.
- [x] Existing phone login, messaging, and persistence paths are preserved;
      regression tests pass, while live account acceptance remains pending.
- [x] Formatting, workspace tests, and strict Clippy pass.
- [x] Optimized native executable and signed macOS bundle are built and verified.
- [ ] Live QR scan and rendered appearance are manually verified when available.

### Extension verification

Implemented through three GPT-6 Luna coding agents at medium effort, with root
integration/review. All 35 offline tests pass: 8 core, 9 desktop, 7 macOS, and 11
Telegram tests. Formatting and strict workspace Clippy pass. QR tests cover
base64url encoding, credential redaction, login update detection, data clearing,
and command priority/cancellation before a network poll. Live QR success, expiry,
DC migration, and 2FA have implementation paths but have not been tested against
an actual Telegram account.

Release outputs on macOS Apple Silicon:

- `target/release/slopogram-desktop`: optimized ARM64 Mach-O executable, about 17 MB.
- `target/release/Slopogram.app`: ad-hoc-signed app bundle. `codesign --verify
  --deep --strict` and `plutil -lint` pass. No credentials or `.env` are bundled.

Final validation commands:

```sh
cargo fmt --all --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
CARGO_NET_OFFLINE=true scripts/bundle-macos.sh
codesign --verify --deep --strict target/release/Slopogram.app
plutil -lint target/release/Slopogram.app/Contents/Info.plist
```

Computer Use again reported that permissions are not granted, so release GUI
appearance and live QR scanning remain unverified. No Telegram messages were sent
and no application credentials were inspected during tool work. Windows/Linux
runtime behavior has not been tested locally.

Follow-up proxy question: fixed the existing macOS proxy parser to handle real
indented `scutil --proxy` output and reject port zero, added regression tests, and
rebuilt/re-signed both artifacts. An explicit `SLOPOGRAM_PROXY=socks5://host:port`
forces an endpoint; when unset the app detects the macOS system SOCKS proxy.

Earlier milestone results below are historical.

## Goal

Log in to an existing Telegram account, open a conversation, send and receive
text messages, close the application, and restore the authenticated session on
the next launch. Target Windows, macOS, and Linux; validate locally on macOS and
use CI for the other desktop platforms.

## Implementation sequence

1. Create a Cargo workspace with `slopogram-core` (application models and state),
   `slopogram-telegram` (grammers and persistent session storage), and
   `slopogram-desktop` (Iced interface).
2. Configure the application's own Telegram API ID and hash through environment
   variables or an ignored `.env` file. Keep account credentials out of Git.
3. Open a durable grammers SQLite session in the user's application data
   directory, prevent concurrent use of the same session, and check authorization
   before requesting a new login code.
4. Implement phone number, login code, and optional two-factor password flows,
   including recoverable errors and returning to the phone step.
5. Load conversations, open recent message history, and send text. Process live
   updates concurrently with commands; merge history, send responses, and updates
   by conversation and message ID so races do not duplicate or discard messages.
6. Persist update state after processing updates and on orderly shutdown. Restore the session
   on launch, catch up after interruptions, and refresh history on opening a chat.
7. Provide loading, sending, connection, empty, and failure states in the GUI.
8. Run formatting, workspace tests, strict Clippy, and a desktop build. Document
   setup and a manual Telegram acceptance test; do not send real messages during
   automated verification.

## Interface direction

A compact messenger with a 300-pixel conversation sidebar and a flexible message
pane. Use cool white (#F5F7FB), white (#FFFFFF), ink (#172638), muted blue-gray
(#62758A), and blue (#2864DC). Use the platform sans-serif font, 14–16 pixel body
text, clear focus states, and restrained message bubbles. Login is a centered,
single-purpose form. Avoid decorative dashboard cards and placeholder chats.

## Acceptance criteria

- [x] Workspace builds and documentation explains API credential setup.
- [x] Phone/code login and optional 2FA are implemented without logging secrets.
- [x] Authorization check and session reuse are implemented (live restart pending).
- [x] Conversation list and recent-history loading are wired to the interface.
- [x] Sending has confirmed/unconfirmed states and preserves failed text for recovery.
- [x] A concurrent live-update stream feeds the interface without manual refresh.
- [x] Tests verify duplicate delivery, history races, edits, and deletion handling.
- [x] Session keys and update state survive reopening the database in offline tests.
- [x] Login retries, connection errors, and explicit reconnect are implemented.
- [x] Formatting, tests, strict Clippy, and the native macOS build pass.
- [ ] Rendered desktop interface is visually verified.
- [ ] Live account login/send/receive/restart is manually verified.

## Boundaries

This milestone is single-account and text-first. It does not include account
creation, media transfer, calls, secret chats, notifications, full history search,
ghost mode, or anti-recall. Session databases contain authorization keys and are
not encrypted at rest in this prototype; use a private application directory and
owner-only permissions on Unix. OS credential-store encryption is follow-up work.
Never commit or share a session database.

## Verification record

Implementation and offline verification completed on 2026-11-15, using Rust
1.99.1 on macOS. Commands passed:

```sh
cargo fmt --all --check
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline -- -D warnings
cargo build -p slopogram-desktop --locked --offline
```

All 20 tests passed: 8 core state/validation tests, 7 interface-state tests, and 5
configuration/session tests. Coverage includes retaining live messages during
history loads, duplicate deliveries, stale edits, deletion races, per-chat message
IDs, failed-send recovery, authentication retry, stale connection events, focused
chat read receipts, durable session keys/cursors, exclusive session locking,
private Unix permissions, and preservation of corrupt databases.

Native startup was attempted with blank API credentials so the check would not
contact Telegram. GUI services were unavailable in the command sandbox, and
Computer Use reported that permissions were not granted. Therefore visual layout
and GUI interactions remain unverified; the test process was stopped afterward.

No live account was used and no Telegram messages were sent. Real login, send,
receive, reconnect, and session reuse still require the README's manual acceptance
checks with the user's own application credentials. The Windows/macOS/Linux CI
workflow is present but has not run remotely. A working build does not prove live
authentication or cross-platform runtime behavior.

## Upstream references

- https://docs.rs/grammers-client/0.12.0/grammers_client/
- https://docs.rs/grammers-session/0.12.0/grammers_session/
- https://docs.rs/iced/0.14.0/iced/
- https://core.telegram.org/api/obtaining_api_id
