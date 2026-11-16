import AppKit
import Combine
import CoreImage.CIFilterBuiltins
import RustBridge
import SlopogramConfig
import SlopogramState
import TGUIKit

@main @MainActor final class TelegramApplication: NSObject, NSApplicationDelegate {
    private let store = Store()
    private var window: NSWindow!
    private var controller: MessengerController!
    private var launched = false

    static func main() {
        let application = NSApplication.shared
        let delegate = TelegramApplication()
        application.delegate = delegate
        application.setActivationPolicy(.regular)
        application.run()
        withExtendedLifetime(delegate) {}
    }
    func applicationDidFinishLaunching(_ notification: Notification) {
        let menu = NSMenu()
        let appItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "Quit Slopogram", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
        menu.addItem(appItem)
        let editItem = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editItem.submenu = editMenu
        menu.addItem(editItem)
        NSApplication.shared.mainMenu = menu
        controller = MessengerController(store: store)
        window = Window(contentRect: NSRect(x: 0, y: 0, width: 1000, height: 700), styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.title = "Slopogram"
        window.minSize = NSSize(width: 760, height: 520)
        window.center()
        window.contentViewController = controller
        window.makeKeyAndOrderFront(nil)
        NSApplication.shared.activate(ignoringOtherApps: true)
        launch()
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard store.hasActiveEngine else { return .terminateNow }
        Task { await store.stop(); sender.reply(toApplicationShouldTerminate: true) }
        return .terminateLater
    }
    private func launch() {
        guard !launched else { return }; launched = true
        if CommandLine.arguments.contains("--ui-preview") || CommandLine.arguments.contains("--smoke-test") {
            store.screen = .chats
            store.chats = [ChatRecord(id: 1, title: "Saved Messages", preview: "Your notes appear here"), ChatRecord(id: 2, title: "Project", preview: "A second conversation")]
            store.selected = 1
            store.timelines[1] = [
                1: MessageRecord(chatId: 1, id: 1, text: "Notes and messages appear here.\nThis message has another line.", sender: "Saved Messages", outgoing: false, sentAt: 1, editedAt: nil),
                2: MessageRecord(chatId: 1, id: 2, text: "A reply from you.", sender: "You", outgoing: true, sentAt: 2, editedAt: nil)
            ]
            store.timelines[2] = [1: MessageRecord(chatId: 2, id: 1, text: "The same numeric ID in another conversation.", sender: "Project", outgoing: false, sentAt: 1, editedAt: nil)]
            if CommandLine.arguments.contains("--smoke-test") {
                DispatchQueue.main.async {
                    self.controller.view.layoutSubtreeIfNeeded()
                    precondition(!self.store.hasActiveEngine && self.controller.fixtureIsRendered, "Offline fixture failed to render")
                    NSApplication.shared.terminate(nil)
                }
            }
            return
        }
        do {
            let values = try SlopogramConfiguration.load(repository: SlopogramConfiguration.developmentRepository)
            guard let raw = values["TELEGRAM_API_ID"], let id = Int32(raw), id > 0, let hash = values["TELEGRAM_API_HASH"], !hash.isEmpty else {
                store.setupMessage = "Set TELEGRAM_API_ID and TELEGRAM_API_HASH in your environment or an external .env file, then restart Slopogram."
                return
            }
            store.start(EngineConfig(apiId: id, apiHash: hash, dataDir: values["SLOPOGRAM_DATA_DIR"], proxy: try SlopogramConfiguration.proxy(environment: values)))
        } catch { store.setupMessage = "Unable to load configuration: \(error.localizedDescription)" }
    }
}
