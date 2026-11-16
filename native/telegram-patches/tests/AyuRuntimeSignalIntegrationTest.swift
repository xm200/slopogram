import Foundation
import AppKit
import SlopogramAyu
import SwiftSignalKit
@main struct Test {
    static func wait(_ predicate: @escaping () -> Bool) async throws {
        for _ in 0..<500 { if predicate() { return }; try await Task.sleep(nanoseconds: 10_000_000) }
        fatalError("Timed out waiting for fixture event")
    }
    static func main() async throws {
        let path = NSTemporaryDirectory() + "ayu-signal-" + UUID().uuidString
        try FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: path) }
        do {
            let features = AyuFeatures(dataDir: URL(fileURLWithPath: path).appendingPathComponent("slopogram-ayu").path)
            try await features.setPolicy(policy: FeaturePolicy(suppressReadReceipts: false, suppressTyping: false, suppressOnlineStatus: false, archiveObservedMessages: false))
            try await features.setLocalCosmetics(value: LocalCosmetics(localPremium: true, emojiStatus: "🌙", profileColor: nil, chatEmoji: []))
        }
        let account = Account(path)
        let runtime = AyuAccountRuntime.account(account)
        runtime.attach(account: account)
        let appearance = Atomic<AyuLocalCosmetics?>(value: nil)
        let appearanceSubscription = runtime.localCosmetics.start(next: { _ = appearance.swap($0) })
        defer { appearanceSubscription.dispose() }
        try await wait { appearance.with { $0?.enabled == true && $0?.emoji == "🌙" } }
        let toggle = NSSelectorFromString("toggleLocalPremium:")
        await MainActor.run { _ = runtime.perform(toggle, with: NSMenuItem()) }
        try await wait { appearance.with { $0?.enabled == false && $0?.emoji == "🌙" } }
        await MainActor.run { _ = runtime.perform(toggle, with: NSMenuItem()) }
        try await wait { appearance.with { $0?.enabled == true && $0?.emoji == "🌙" } }
        await MainActor.run {
            let purple = NSMenuItem()
            purple.representedObject = NSNumber(value: UInt32(0x9a69df))
            _ = runtime.perform(NSSelectorFromString("chooseProfileColor:"), with: purple)
            runtime.setChatEmoji(trigger: "🌈", fileId: 42)
            runtime.setChatEmoji(trigger: "🦋", fileId: 43)
        }
        try await wait {
            appearance.with {
                $0?.profileColor == 0x9a69df && $0?.chatEmoji.map(\.fileId) == [42, 43] && $0?.emoji == "🌙"
            }
        }
        await MainActor.run { runtime.setChatEmoji(trigger: "🌈", fileId: nil) }
        try await wait { appearance.with { $0?.chatEmoji.map(\.fileId) == [43] } }
        let rows = Atomic<[AyuRetainedText]>(value: [])
        let other = Atomic<[AyuRetainedText]>(value: [])
        let subscription = runtime.retainedHistory(chat: 44).start(next: { _ = rows.swap($0) })
        let otherSubscription = runtime.retainedHistory(chat: 45).start(next: { _ = other.swap($0) })
        defer { subscription.dispose(); otherSubscription.dispose(); AyuAccountRuntime.release(account) }
        try await wait { account.postbox.source.starts > 0 && account.stateManager.source.starts > 0 }
        account.postbox.source.emit([Message(chat: 44, id: 7, text: "observed fixture")])
        await AyuAccountRuntime.flushAll()
        account.stateManager.source.emit(MessageDeletionEvent(ids: [.messageId(MessageId(namespace: 0, peerId: PeerId(44), id: 7))], origin: .remoteConfirmed))
        try await wait { rows.with { $0.contains(where: { $0.chatId == 44 && $0.messageId == 7 && $0.text == "observed fixture" }) } }
        precondition(other.with { $0.isEmpty })
        let staleCallbacks = Atomic<Int>(value: 0)
        let stale = runtime.retainedHistory(chat: 44).start(next: { _ in _ = staleCallbacks.modify { $0 + 1 } })
        stale.dispose()
        let switchedReady = Atomic<Bool>(value: false)
        let switched = runtime.retainedHistory(chat: 45).start(next: { value in precondition(value.isEmpty); _ = switchedReady.swap(true) })
        try await wait { switchedReady.with { $0 } }
        precondition(staleCallbacks.with { $0 == 0 })
        switched.dispose()
        account.stateManager.source.emit(MessageDeletionEvent(ids: [.messageId(MessageId(namespace: 0, peerId: PeerId(44), id: 7))], origin: .interactiveLocal))
        try await wait { rows.with { $0.isEmpty } }
        account.stateManager.source.emit(MessageDeletionEvent(ids: [.messageId(MessageId(namespace: 0, peerId: PeerId(44), id: 7))], origin: .remoteConfirmed))
        await AyuAccountRuntime.flushAll()
        let verified = Atomic<Bool>(value: false)
        let verify = runtime.retainedHistory(chat: 44).start(next: { value in precondition(value.isEmpty); _ = verified.swap(true) })
        try await wait { verified.with { $0 } }
        verify.dispose()
        precondition(rows.with { $0.isEmpty })
        account.postbox.source.emit([Message(chat: 44, id: 8, text: "second fixture")])
        await AyuAccountRuntime.flushAll()
        account.stateManager.source.emit(MessageDeletionEvent(ids: [.messageId(MessageId(namespace: 0, peerId: PeerId(44), id: 8))], origin: .remoteConfirmed))
        try await wait { rows.with { $0.contains(where: { $0.messageId == 8 }) } }
        let cleared = await withCheckedContinuation { continuation in runtime.clearHistoryData { continuation.resume(returning: $0) } }
        precondition(cleared)
        try await wait { rows.with { $0.isEmpty } }
        precondition(account.postbox.source.starts == 1)
        account.postbox.source.emit([Message(chat: 44, id: 9, text: "after clear")])
        await AyuAccountRuntime.flushAll()
        account.stateManager.source.emit(MessageDeletionEvent(ids: [.messageId(MessageId(namespace: 0, peerId: PeerId(44), id: 9))], origin: .remoteConfirmed))
        try await wait { rows.with { $0.contains(where: { $0.messageId == 9 && $0.text == "after clear" }) } }
        print("Ayu real signal and Rust retention check passed")
    }
}
