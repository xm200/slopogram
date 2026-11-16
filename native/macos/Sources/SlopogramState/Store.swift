import Foundation
import Combine
import RustBridge

public func displayMessage(for error: Error) -> String {
    if case EngineError.Failed(let message) = error { return message }
    return error.localizedDescription
}

@MainActor public final class Store: ObservableObject {
    public enum Screen { case starting, phone, code, password(String), qr(String), chats }
    @Published public var screen: Screen = .starting
    @Published public var chats: [ChatRecord] = []
    @Published public var selected: Int64?
    @Published public var timelines: [Int64: [Int32: MessageRecord]] = [:]
    @Published public var drafts: [Int64: String] = [:]
    @Published public var error: String?
    @Published public var setupMessage: String?
    @Published public var connected = true
    @Published public var focused = false
    @Published public private(set) var isReconnecting = false
    @Published public var failed: [UInt64: (Int64, String)] = [:]
    @Published public private(set) var pending: [UInt64: (Int64, String)] = [:]
    private var request: UInt64 = 0
    private var pump: Task<Void, Never>?
    private var generation = 0
    private var engine: Engine?
    private var shutdownTask: Task<Void, Never>?
    public var hasActiveEngine: Bool { engine != nil || shutdownTask != nil }
    private var config: EngineConfig?
    private var historyReady: Set<Int64> = []

    public init() {}

    public func start(_ config: EngineConfig) {
        guard engine == nil, shutdownTask == nil else { return }
        self.config = config
        connected = true
        do {
            let engine = try Engine(config: config)
            self.engine = engine
            generation += 1
            let currentGeneration = generation
            pump = Task { [weak self] in
                while !Task.isCancelled, let event = await engine.nextEvent() {
                    guard let self, self.generation == currentGeneration else { return }
                    self.apply(event)
                }
            }
        } catch {
            connected = false
            setupMessage = "\(displayMessage(for: error)) Check your external .env file or environment settings, then restart Slopogram."
        }
    }
    static func movePendingToFailed(pending: inout [UInt64: (Int64, String)], failed: inout [UInt64: (Int64, String)]) {
        for (id, item) in pending { failed[id] = item }
        pending.removeAll()
    }
    public func stop() async {
        if let shutdownTask {
            await shutdownTask.value
            return
        }
        generation += 1
        historyReady.removeAll()
        Self.movePendingToFailed(pending: &pending, failed: &failed)
        let engine = engine
        self.engine = nil
        let task = Task { try? await engine?.shutdown() }
        shutdownTask = Task { _ = await task.value }
        await shutdownTask?.value
        pump?.cancel()
        pump = nil
        shutdownTask = nil
    }
    public func reconnect() {
        guard let config, !isReconnecting else { return }
        isReconnecting = true
        Task { await stop(); start(config); isReconnecting = false }
    }
    public func command(_ body: @escaping (Engine) async throws -> Void) {
        guard let engine else { return }
        let currentGeneration = generation
        Task { do { try await body(engine) } catch { if self.generation == currentGeneration { self.error = displayMessage(for: error) } } }
    }
    public func select(_ id: Int64) {
        selected = id
        historyReady.remove(id)
        command { try await $0.openChat(chatId: id) }
    }
    public func markRead() { if focused, let selected, historyReady.contains(selected) { command { try await $0.markRead(chatId: selected) } } }
    public func recover(_ id: UInt64) {
        guard let (chat, text) = failed.removeValue(forKey: id) else { return }
        let current = drafts[chat] ?? ""
        drafts[chat] = current.isEmpty ? text : current + "\n" + text
    }
    public func send() {
        guard let id = selected, let text = drafts[id], !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        request += 1; let number = request
        pending[number] = (id, text); drafts[id] = ""
        guard let engine else { pending.removeValue(forKey: number); failed[number] = (id, text); return }
        let currentGeneration = generation
        Task {
            do { try await engine.sendMessage(requestId: number, chatId: id, text: text) }
            catch { guard generation == currentGeneration else { return }; if let item = pending.removeValue(forKey: number) { failed[number] = item }; self.error = displayMessage(for: error) }
        }
    }
    private func merge(_ message: MessageRecord) {
        timelines[message.chatId, default: [:]][message.id] = message
    }
    func apply(_ event: ClientEvent) {
        switch event {
        case .authPhone: screen = .phone
        case .authCode: screen = .code
        case .authPassword(let hint): screen = .password(hint)
        case .authQr(let url, _): screen = .qr(url)
        case .authorized:
            screen = .chats
            command { try await $0.loadChats() }
            if let selected { command { try await $0.openChat(chatId: selected) } }
        case .chats(let chats): self.chats = chats
        case .chatUpdated(let chat): if let i = chats.firstIndex(where: { $0.id == chat.id }) { chats[i] = chat } else { chats.insert(chat, at: 0) }
        case .history(let chat, let messages):
            timelines[chat] = Dictionary(uniqueKeysWithValues: messages.map { ($0.id, $0) })
            historyReady.insert(chat)
            if selected == chat { markRead() }
        case .message(let message): merge(message); if selected == message.chatId { markRead() }
        case .deleted(let chatId, let ids):
            for chat in chatId.map({ [$0] }) ?? Array(timelines.keys.filter { $0 > -1_000_000_000_000 }) {
                for id in ids { timelines[chat]?.removeValue(forKey: id) }
            }
        case .sent(let requestId, let message): pending.removeValue(forKey: requestId); merge(message)
        case .sendConfirmed(let requestId): pending.removeValue(forKey: requestId)
        case .connection(let connected): self.connected = connected
        case .failure(let operation, let message):
            if operation.hasPrefix("send:"), let id = UInt64(operation.dropFirst(5)), let (chat, text) = pending.removeValue(forKey: id) {
                failed[id] = (chat, text)
            }
            error = message
        case .fatal(let message): connected = false; error = message
        case .stopped: connected = false
        }
    }
}
