import Foundation
struct AyuRetainedText: Equatable {
    let chatId: Int64
    let messageId: Int32
    let timestamp: Int64
    let text: String
    let sender: String
    let authorPeerId: Int64?
    let outgoing: Bool
}
struct TestRow: Comparable, Identifiable {
    let stableId: String
    let timestamp: Int64
    let text: String
    static func < (lhs: TestRow, rhs: TestRow) -> Bool {
        (lhs.timestamp, lhs.stableId) < (rhs.timestamp, rhs.stableId)
    }
}
@main struct Test {
    static func item(_ chat: Int64, _ id: Int32, _ time: Int64, _ text: String = "kept") -> AyuRetainedText {
        AyuRetainedText(chatId: chat, messageId: id, timestamp: time, text: text, sender: "A", authorPeerId: nil, outgoing: false)
    }
    static func row(_ item: AyuRetainedText) -> TestRow {
        TestRow(stableId: "ayu.deleted.\(item.chatId).\(item.messageId)", timestamp: item.timestamp, text: item.text)
    }
    static func main() {
        let complete = AyuHistoryWindow(earliest: nil, latest: nil, hasEarlierGap: false, hasLaterGap: false)
        let retained = ayuProjectDeletedText([item(1, 7, 100), item(2, 7, 100)], chatId: 1, liveIds: [], window: complete)
        precondition(retained.count == 1 && retained[0].chatId == 1)
        let old = [TestRow(stableId: "live.1.7", timestamp: 100, text: "live")]
        let new = retained.map(row)
        let (removed, inserted, updated) = proccessEntries(old, right: new) { $0 }
        precondition(removed == [0] && inserted.map(\.0) == [0] && updated.isEmpty)
        let (offRemoved, offInserted, _) = proccessEntries(new, right: [TestRow]()) { $0 }
        precondition(offRemoved == [0] && offInserted.isEmpty)
        precondition(ayuProjectDeletedText([item(1, 7, 100)], chatId: 1, liveIds: [7], window: complete).isEmpty)
        let middle = AyuHistoryWindow(earliest: .init(timestamp: 100, messageId: 1), latest: .init(timestamp: 200, messageId: 3), hasEarlierGap: true, hasLaterGap: true)
        precondition(ayuProjectDeletedText([item(1, 1, 90), item(1, 2, 150), item(1, 3, 210)], chatId: 1, liveIds: [], window: middle).map(\.messageId) == [2])
        precondition(ayuProjectDeletedText([item(1, 0, 100), item(1, 4, 200)], chatId: 1, liveIds: [], window: middle).isEmpty)
        let newestOpen = AyuHistoryWindow(earliest: .init(timestamp: 100, messageId: 1), latest: .init(timestamp: 200, messageId: 3), hasEarlierGap: true, hasLaterGap: false)
        precondition(ayuProjectDeletedText([item(1, 3, 210)], chatId: 1, liveIds: [], window: newestOpen).count == 1)
        let oldMixed = [TestRow(stableId: "live.1.1", timestamp: 10, text: "a"), TestRow(stableId: "ayu.deleted.1.2", timestamp: 20, text: "b"), TestRow(stableId: "live.1.3", timestamp: 30, text: "c")]
        let newMixed = [oldMixed[0], oldMixed[2]]
        let (midRemoved, midInserted, _) = proccessEntries(oldMixed, right: newMixed) { $0 }
        precondition(midRemoved == [1] && midInserted.isEmpty)
        print("Ayu projected history transition checks passed")
    }
}
