import SwiftUI
import SlopogramState
import RustBridge
import SlopogramConfig

@MainActor final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var store: Store?
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard let store, store.hasActiveEngine else { return .terminateNow }
        Task { await store.stop(); sender.reply(toApplicationShouldTerminate: true) }
        return .terminateLater
    }
}

@main struct SlopogramApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var store = Store()
    @State private var launched = false
    var body: some Scene {
        WindowGroup {
            ContentView(store: store)
                .task { delegate.store = store; launch() }
        }
        .commands { CommandGroup(replacing: .appTermination) { Button("Quit Slopogram") { NSApplication.shared.terminate(nil) }.keyboardShortcut("q") } }
    }
    @MainActor private func launch() {
        guard !launched else { return }; launched = true
        if CommandLine.arguments.contains("--ui-preview") || CommandLine.arguments.contains("--smoke-test") {
            store.screen = .chats
            store.chats = [ChatRecord(id: 1, title: "Saved Messages", preview: "Your notes appear here")]
            store.selected = 1
            if CommandLine.arguments.contains("--smoke-test") {
                DispatchQueue.main.async {
                    precondition(self.store.chats.count == 1 && self.store.selected == 1 && !self.store.hasActiveEngine, "Offline fixture failed to initialize")
                    NSApplication.shared.terminate(nil)
                }
            }
            return
        }
        do {
            let values = try SlopogramConfiguration.load(repository: SlopogramConfiguration.developmentRepository)
            guard let raw = values["TELEGRAM_API_ID"], let id = Int32(raw), id > 0,
                  let hash = values["TELEGRAM_API_HASH"], !hash.isEmpty else {
                store.setupMessage = "Set TELEGRAM_API_ID and TELEGRAM_API_HASH in your environment or an external .env file, then restart Slopogram."
                return
            }
            let config = EngineConfig(apiId: id, apiHash: hash, dataDir: values["SLOPOGRAM_DATA_DIR"], proxy: try SlopogramConfiguration.proxy(environment: values))
            store.start(config)
        } catch { store.setupMessage = "Unable to load configuration: \(error.localizedDescription)" }
    }
}
