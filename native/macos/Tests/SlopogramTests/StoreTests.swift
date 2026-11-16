import XCTest
import RustBridge
@testable import SlopogramState

@MainActor final class StoreTests: XCTestCase {
    func testBackendErrorMessageIsDisplayed() {
        XCTAssertEqual(displayMessage(for: EngineError.Failed(message: "Check your connection.")), "Check your connection.")
    }
    func testAuthoritativeHistoryReplacesTimeline() {
        let store = Store()
        let message = MessageRecord(chatId: 1, id: 7, text: "a", sender: "A", outgoing: false, sentAt: 1, editedAt: nil)
        store.apply(.message(message: message))
        store.apply(.history(chatId: 1, messages: []))
        XCTAssertNil(store.timelines[1]?[7])
    }
    func testPendingBecomesExplicitFailureOnStop() {
        var pending: [UInt64: (Int64, String)] = [7: (1, "unconfirmed")]
        var failed: [UInt64: (Int64, String)] = [:]
        let store = Store()
        store.drafts[1] = "fresh draft"
        Store.movePendingToFailed(pending: &pending, failed: &failed)
        XCTAssertTrue(pending.isEmpty)
        XCTAssertEqual(failed[7]?.1, "unconfirmed")
        XCTAssertEqual(store.drafts[1], "fresh draft")
        store.failed = failed
        store.recover(7)
        XCTAssertEqual(store.drafts[1], "fresh draft\nunconfirmed")
    }
    func testFailedMessageRecoveryKeepsExistingDraft() {
        let store = Store()
        store.drafts[1] = "current"
        store.failed[2] = (1, "failed")
        store.recover(2)
        XCTAssertEqual(store.drafts[1], "current\nfailed")
        XCTAssertNil(store.failed[2])
    }
    func testDeletionRemovesOnlyTargetChat() {
        let store = Store()
        let message = MessageRecord(chatId: 1, id: 7, text: "a", sender: "A", outgoing: false, sentAt: 1, editedAt: nil)
        store.apply(.message(message: message))
        store.apply(.deleted(chatId: 1, ids: [7]))
        XCTAssertNil(store.timelines[1]?[7])
    }
    func testSameMessageIdInDifferentChatsStaysSeparate() {
        let store = Store()
        let first = MessageRecord(chatId: 1, id: 7, text: "first", sender: "A", outgoing: false, sentAt: 1, editedAt: nil)
        let second = MessageRecord(chatId: 2, id: 7, text: "second", sender: "B", outgoing: false, sentAt: 1, editedAt: nil)
        store.apply(.message(message: first))
        store.apply(.message(message: second))
        store.apply(.deleted(chatId: 1, ids: [7]))
        XCTAssertNil(store.timelines[1]?[7])
        XCTAssertEqual(store.timelines[2]?[7]?.text, "second")
    }
}
