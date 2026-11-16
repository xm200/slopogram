import Foundation
import SwiftSignalKit
struct PeerId { let namespace: Int32; let value: Int64; init(_ value: Int64, namespace: Int32 = 0) { self.value = value; self.namespace = namespace }; func toInt64() -> Int64 { value } }
struct MessageId { let namespace: Int32; let peerId: PeerId; let id: Int32 }
enum Namespaces { enum Message { static let Cloud: Int32 = 0 }; enum Peer { static let CloudUser: Int32 = 0; static let CloudGroup: Int32 = 1; static let CloudChannel: Int32 = 2 } }
enum DeletedMessageId { case messageId(MessageId), global(Int32) }
enum MessageDeletionOrigin { case remoteConfirmed, interactiveLocal }
struct MessageDeletionEvent { let ids: [DeletedMessageId]; let origin: MessageDeletionOrigin }
struct AyuChatEmoji: Equatable { let trigger: String; let fileId: Int64 }
struct AyuLocalCosmetics: Equatable { let enabled: Bool; let emoji: String?; var profileColor: UInt32? = nil; var chatEmoji: [AyuChatEmoji] = [] }
struct AyuRetainedText: Equatable { let chatId: Int64; let messageId: Int32; let timestamp: Int64; let text: String; let sender: String; let authorPeerId: Int64?; let outgoing: Bool }
final class TestSource<T> {
    private let pipe = ValuePipe<T>()
    private let lock = NSLock()
    private var count = 0
    var starts: Int { lock.lock(); defer { lock.unlock() }; return count }
    func signal() -> Signal<T, NoError> {
        return Signal { subscriber in
            self.lock.lock(); self.count += 1; self.lock.unlock()
            return self.pipe.signal().start(next: { subscriber.putNext($0) })
        }
    }
    func emit(_ value: T) { pipe.putNext(value) }
}
final class StateManager { let source = TestSource<MessageDeletionEvent>(); var messageDeletionEvents: Signal<MessageDeletionEvent, NoError> { source.signal() } }
final class StubPostbox { let source = TestSource<[Message]>(); func committedMessageVersions() -> Signal<[Message], NoError> { source.signal() } }
final class StubAuthor { let displayTitle: String; let id: PeerId; init(_ title: String, _ id: PeerId) { displayTitle = title; self.id = id } }
final class Message {
    let id: MessageId; let text: String; let author: StubAuthor?; let timestamp: Int32; let editedTime: Int32?
    init(chat: Int64, id: Int32, text: String) { self.id = MessageId(namespace: 0, peerId: PeerId(chat), id: id); self.text = text; author = StubAuthor("A", PeerId(10)); timestamp = 100; editedTime = nil }
    func effectivelyIncoming(_ peer: PeerId) -> Bool { true }
}
final class Account { let basePath: String; let stateManager = StateManager(); let postbox = StubPostbox(); let peerId = PeerId(1); init(_ path: String) { basePath = path } }
enum SlopogramStreamerMode { static var isActive: Bool { false }; static func whenDisabled(_ action: @escaping () -> Void) { action() } }
