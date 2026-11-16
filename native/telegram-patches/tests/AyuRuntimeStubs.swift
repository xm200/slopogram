import Foundation
protocol Disposable { func dispose() }
final class TestDisposable: Disposable { let action: () -> Void; init(_ action: @escaping () -> Void) { self.action = action }; func dispose() { action() } }
final class ActionDisposable: Disposable { let action: () -> Void; init(_ action: @escaping () -> Void) { self.action = action }; func dispose() { action() } }
struct PeerId { var namespace: Int32 = 0; func toInt64() -> Int64 { 0 } }
struct MessageId { var namespace: Int32; var peerId: PeerId; var id: Int32 }
enum Namespaces { enum Message { static let Cloud: Int32 = 0 }; enum Peer { static let CloudUser: Int32 = 0; static let CloudGroup: Int32 = 1; static let CloudChannel: Int32 = 2 } }
enum DeletedMessageId { case messageId(MessageId), global(Int32) }
enum MessageDeletionOrigin { case remoteConfirmed, interactiveLocal }
struct MessageDeletionEvent { let ids: [DeletedMessageId]; let origin: MessageDeletionOrigin }
enum NoError: Error {}
final class Subscriber<T> { func putNext(_ value: T) {}; func putCompletion() {} }
final class Signal<T, E: Error> {
    private let lock = NSLock()
    private var startCount = 0
    private var disposalCount = 0
    var starts: Int { lock.lock(); defer { lock.unlock() }; return startCount }
    var disposals: Int { lock.lock(); defer { lock.unlock() }; return disposalCount }
    init() {}
    init(_ generator: @escaping (Subscriber<T>) -> Disposable) {}
    static func single(_ value: T) -> Signal<T, E> { Signal() }
    func start(next: @escaping (T) -> Void) -> Disposable {
        lock.lock(); startCount += 1; lock.unlock()
        return TestDisposable { self.lock.lock(); self.disposalCount += 1; self.lock.unlock() }
    }
}
final class ValuePromise<T> { init(_ value: T, ignoreRepeated: Bool) {}; func get() -> Signal<T, NoError> { Signal() }; func set(_ value: T) {} }
final class Atomic<T> { private let lock = NSLock(); private var value: T; init(value: T) { self.value = value }; func with<R>(_ f: (T) -> R) -> R { lock.lock(); defer { lock.unlock() }; return f(value) }; func swap(_ v: T) -> T { lock.lock(); defer { lock.unlock() }; let old = value; value = v; return old } }
infix operator |> : AdditionPrecedence
func |> <T, R>(value: T, transform: (T) -> R) -> R { transform(value) }
func mapToSignal<T, R, E>(_ f: @escaping (T) -> Signal<R, E>) -> (Signal<T, E>) -> Signal<R, E> { { _ in Signal<R, E>() } }
func distinctUntilChanged<T: Equatable, E>(_ signal: Signal<T, E>) -> Signal<T, E> { signal }
struct AyuChatEmoji: Equatable { let trigger: String; let fileId: Int64 }
struct AyuLocalCosmetics: Equatable { let enabled: Bool; let emoji: String?; var profileColor: UInt32? = nil; var chatEmoji: [AyuChatEmoji] = [] }
struct AyuRetainedText: Equatable { let chatId: Int64; let messageId: Int32; let timestamp: Int64; let text: String; let sender: String; let authorPeerId: Int64?; let outgoing: Bool }
final class StateManager { let messageDeletionEvents = Signal<MessageDeletionEvent, NoError>() }
final class StubPostbox { let versions = Signal<[Message], NoError>(); func committedMessageVersions() -> Signal<[Message], NoError> { versions } }
final class StubAuthor { let displayTitle = ""; let id = PeerId() }
final class Message { let id = MessageId(namespace: 0, peerId: PeerId(), id: 1); let text = ""; let author: StubAuthor? = nil; let timestamp: Int32 = 1; let editedTime: Int32? = nil; func effectivelyIncoming(_ peer: PeerId) -> Bool { true } }
final class Account { let basePath: String; let stateManager = StateManager(); let postbox = StubPostbox(); let peerId = PeerId(); init(_ path: String) { basePath = path } }
enum SlopogramStreamerMode { static var isActive: Bool { false }; static func whenDisabled(_ action: @escaping () -> Void) { action() } }
