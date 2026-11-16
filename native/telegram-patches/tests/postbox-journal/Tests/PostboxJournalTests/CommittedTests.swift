import Foundation
import XCTest
import Postbox
import SwiftSignalKit

final class CommittedTests: XCTestCase {
    func testCommittedSnapshotsAndDisposal() {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: dir) }
        let ns: MessageId.Namespace = 0
        let seed = SeedConfiguration(globalMessageIdsPeerIdNamespaces: [], initializeChatListWithHole: (topLevel: nil, groups: nil), messageHoles: [:], upgradedMessageHoles: [:], messageThreadHoles: { _, _ in nil }, existingMessageTags: [], messageTagsWithSummary: [], messageTagsWithThreadSummary: [], existingGlobalMessageTags: [], peerNamespacesRequiringMessageTextIndex: [], peerSummaryCounterTags: { _, _ in PeerSummaryCounterTags() }, peerSummaryIsThreadBased: { _, _ in (false, false) }, additionalChatListIndexNamespace: nil, messageNamespacesRequiringGroupStatsValidation: [], defaultMessageNamespaceReadStates: [:], chatMessagesNamespaces: [ns], getGlobalNotificationSettings: { _ in nil }, defaultGlobalNotificationSettings: PostboxGlobalNotificationSettings(defaultIncludePeer: { _ in true }), mergeMessageAttributes: { _, _ in }, decodeMessageThreadInfo: { _ in nil }, decodeAutoremoveTimeout: { _ in nil }, decodeDisplayPeerAsRegularChat: { _ in false }, isPeerUpgradeMessage: { _ in false }, automaticThreadIndexInfo: { _, _ in nil }, customTagsFromAttributes: { _ in [] }, displaySavedMessagesAsTopicListPreferencesKey: ValueBoxKey(length: 0))
        let params = ValueBoxEncryptionParameters(forceEncryptionIfNoSet: false, key: .init(data: Data(repeating: 0, count: 32))!, salt: .init(data: Data(repeating: 0, count: 16))!)
        let opened = expectation(description: "opened")
        var postbox: Postbox?
        let opening = openPostbox(basePath: dir.path, seedConfiguration: seed, encryptionParameters: params, timestampForAbsoluteTimeBasedOperations: 0, isMainProcess: true, isTemporary: false, isReadOnly: false, useCopy: false, useCaches: true, removeDatabaseOnError: false).start(next: { result in if case let .postbox(value) = result { postbox = value; opened.fulfill() } })
        wait(for: [opened], timeout: 20)
        do {
        guard let postbox else { return XCTFail("failed to open") }
        let id = MessageId(peerId: PeerId(namespace: ._internalFromInt32Value(0), id: ._internalFromInt64Value(123)), namespace: ns, id: 1)
        func message(_ text: String) -> StoreMessage { StoreMessage(id: id, customStableId: nil, globallyUniqueId: nil, groupingKey: nil, threadId: nil, timestamp: 1, flags: [], tags: [], globalTags: [], localTags: [], forwardInfo: nil, authorId: nil, text: text, attributes: [], media: []) }
        let lock = NSLock(); var snapshots: [String] = []; var reads: [String] = []
        let seen = expectation(description: "callbacks"); seen.expectedFulfillmentCount = 3
        let observed = postbox.committedMessageVersions().start(next: { messages in
            lock.lock(); snapshots += messages.map(\.text); lock.unlock()
            let _ = postbox.transaction { $0.getMessage(id)?.text }.start(next: { value in lock.lock(); reads.append(value ?? "missing"); lock.unlock(); seen.fulfill() })
        })
        let done = expectation(description: "transaction")
        let tx = postbox.transaction { t in
            _ = t.addMessages([message("one")], location: .Random)
            t.updateMessage(id) { _ in .update(message("two")) }
            t.updateMessage(id) { _ in .update(message("three")) }
        }.start(completed: { done.fulfill() })
        wait(for: [done, seen], timeout: 20)
        lock.lock(); let actual = snapshots; let committed = reads; lock.unlock()
        XCTAssertEqual(actual, ["one", "one", "two", "two", "three"])
        XCTAssertEqual(committed, ["three", "three", "three"])
        observed.dispose()
        let next = expectation(description: "after disposal")
        let tx2 = postbox.transaction { t in t.updateMessage(id) { _ in .update(message("four")) } }.start(completed: { next.fulfill() })
        wait(for: [next], timeout: 10)
        lock.lock(); let final = snapshots; lock.unlock(); XCTAssertEqual(final, actual)
        tx.dispose(); tx2.dispose(); opening.dispose()
        let storageReady = expectation(description: "storage ready")
        let storage = postbox.mediaBox.storageBox.totalSize().start(next: { _ in storageReady.fulfill() })
        wait(for: [storageReady], timeout: 10)
        storage.dispose()
        }
        postbox = nil
        Postbox.sharedQueue.sync {}
    }
}
