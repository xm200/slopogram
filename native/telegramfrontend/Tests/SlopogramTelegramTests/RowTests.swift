import XCTest
@testable import SlopogramTelegram

final class RowTests: XCTestCase {
    private func row(_ id: Int, _ text: String, selected: Bool = false) -> TelegramRowData {
        TelegramRowData(id: AnyHashable(id), title: "Chat", text: text, outgoing: false, conversation: true, selected: selected)
    }
    func testIdentityDiffDetectsInsertionDeletionAndEdit() {
        let delta = TelegramRowDelta(old: [row(1, "old"), row(2, "remove")], new: [row(1, "new"), row(3, "insert")])
        XCTAssertEqual(delta.removed, [1])
        XCTAssertEqual(delta.inserted, [1])
        XCTAssertEqual(delta.changed, [0])
    }
    func testFollowLatestOnlyInitiallyOrAtBottom() {
        XCTAssertTrue(shouldFollowLatest(oldCount: 0, newCount: 3, wasAtBottom: false))
        XCTAssertTrue(shouldFollowLatest(oldCount: 3, newCount: 4, wasAtBottom: true))
        XCTAssertFalse(shouldFollowLatest(oldCount: 3, newCount: 4, wasAtBottom: false))
        XCTAssertFalse(shouldFollowLatest(oldCount: 3, newCount: 3, wasAtBottom: true))
    }
    func testSelectionChangeRefreshesExistingRow() {
        let delta = TelegramRowDelta(old: [row(1, "message")], new: [row(1, "message", selected: true)])
        XCTAssertEqual(delta.changed, [0])
        XCTAssertTrue(delta.inserted.isEmpty)
        XCTAssertTrue(delta.removed.isEmpty)
    }
}
