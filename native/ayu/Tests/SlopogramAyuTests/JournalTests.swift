import Foundation
import XCTest
@testable import SlopogramAyu

final class JournalTests: XCTestCase {
    func testJournalRoundTrip() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let features = AyuFeatures(dataDir: root.path)
        let initial = try await features.policy()
        XCTAssertTrue(initial.archiveObservedMessages)
        try await features.setPolicy(policy: FeaturePolicy(suppressReadReceipts: true, suppressTyping: false, suppressOnlineStatus: false, archiveObservedMessages: true))
        try await features.observe(message: AyuMessage(chatId: 12, id: 34, text: "hello", sender: "A", outgoing: false, sentAt: 1, editedAt: nil, globalIdEligible: true, authorPeerId: 42))
        try await features.recordDeletion(chatId: 12, ids: [34])
        let history = try await features.history(chatId: 12)
        XCTAssertEqual(history.count, 1)
        XCTAssertTrue(history[0].deleted)
        XCTAssertEqual(history[0].versions[0].text, "hello")
        try await features.clearHistory()
        let cleared = try await features.history(chatId: 12)
        XCTAssertTrue(cleared.isEmpty)
    }
    func testDesktopThemeDecoderFromPlainColors() async throws {
        let file = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".tdesktop-theme")
        defer { try? FileManager.default.removeItem(at: file) }
        try "windowBg: #101010; windowBgActive: #abcdef80;".write(to: file, atomically: true, encoding: .utf8)
        let theme = try await decodeDesktopTheme(path: file.path)
        XCTAssertTrue(theme.isDark)
        XCTAssertNil(theme.wallpaper)
        XCTAssertTrue(theme.palette.contains("basicAccent = #abcdef:0.501961"))
    }

    func testLocalCosmeticsRoundTrip() async throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let features = AyuFeatures(dataDir: root.path)
        let initial = try await features.localCosmetics()
        XCTAssertFalse(initial.localPremium)
        XCTAssertNil(initial.emojiStatus)
        try await features.setLocalCosmetics(value: LocalCosmetics(localPremium: true, emojiStatus: "👩‍💻", profileColor: 0x123456, chatEmoji: [LocalChatEmoji(trigger: "⭐", fileId: 111)]))
        try await features.clearHistory()
        let stored = try await features.localCosmetics()
        XCTAssertTrue(stored.localPremium)
        XCTAssertEqual(stored.emojiStatus, "👩‍💻")
        XCTAssertEqual(stored.profileColor, 0x123456)
        XCTAssertEqual(stored.chatEmoji.first?.fileId, 111)
    }

}
