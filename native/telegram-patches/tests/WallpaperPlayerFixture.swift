import AppKit
import AVFoundation

private struct FixtureError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

@MainActor
private final class Fixture: NSObject, NSApplicationDelegate {
    let clip: URL
    var exitCode: Int32 = 1

    init(clip: URL) { self.clip = clip }

    func applicationDidFinishLaunching(_ notification: Notification) {
        Task { @MainActor in
            do {
                try await run()
                print("Wallpaper first frame, loop, low-power poster, resume and hidden teardown passed.")
                exitCode = 0
            } catch {
                fputs("Wallpaper fixture failed: \(error)\n", stderr)
            }
            NSApp.stop(nil)
            NSApp.postEvent(NSEvent.otherEvent(with: .applicationDefined, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: 0, context: nil, subtype: 0, data1: 0, data2: 0)!, atStart: false)
        }
    }

    private func wait(for seconds: Double, _ description: String, until condition: () -> Bool) async throws {
        let deadline = ProcessInfo.processInfo.systemUptime + seconds
        while !condition() {
            if ProcessInfo.processInfo.systemUptime >= deadline { throw FixtureError(description) }
            try await Task.sleep(nanoseconds: 20_000_000)
        }
    }

    private func run() async throws {
        let frame = NSRect(x: 90, y: 120, width: 320, height: 240)
        let window = NSWindow(contentRect: frame, styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let poster = NSView(frame: frame)
        poster.wantsLayer = true
        poster.layer?.backgroundColor = NSColor.darkGray.cgColor
        window.contentView = poster
        let overlay = NSView(frame: poster.bounds)
        poster.addSubview(overlay)
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
        try await wait(for: 6, "window did not become active/key/visible") {
            window.isKeyWindow && NSApp.isActive && window.occlusionState.contains(.visible)
        }

        var lowPower = false
        let wallpaper = SlopogramWallpaperPlayer(container: overlay, lowPower: { lowPower })
        func layer() -> AVPlayerLayer? { overlay.layer?.sublayers?.compactMap { $0 as? AVPlayerLayer }.first }
        func ready() -> Bool { layer()?.isReadyForDisplay == true && layer()?.isHidden == false }
        wallpaper.setSource(clip)
        wallpaper.setVisible(true)
        try await wait(for: 10, "first frame unavailable", until: ready)
        var previous = 0.0
        try await wait(for: 5, "clip did not loop") {
            let time = layer()?.player?.currentTime().seconds ?? 0
            defer { previous = time }
            return time + 0.3 < previous
        }
        lowPower = true
        NotificationCenter.default.post(name: .NSProcessInfoPowerStateDidChange, object: nil)
        try await wait(for: 2, "video remained above poster in low power mode") { layer() == nil }
        guard overlay.superview === poster, poster.layer?.backgroundColor == NSColor.darkGray.cgColor else {
            throw FixtureError("poster missing during fallback")
        }
        lowPower = false
        NotificationCenter.default.post(name: .NSProcessInfoPowerStateDidChange, object: nil)
        try await wait(for: 10, "video did not resume", until: ready)
        wallpaper.setVisible(false)
        guard layer() == nil else { throw FixtureError("hidden chat retained player layer") }
    }
}

@main
struct WallpaperPlayerFixture {
    @MainActor static func main() {
        let app = NSApplication.shared
        let fixture = Fixture(clip: URL(fileURLWithPath: CommandLine.arguments[1]))
        app.setActivationPolicy(.regular)
        app.delegate = fixture
        app.run()
        exit(fixture.exitCode)
    }
}
