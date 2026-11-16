import Foundation
public enum NoError: Error {}
public protocol Disposable { func dispose() }
public final class EmptyDisposable: Disposable { public init() {} ; public func dispose() {} }
public final class MetaDisposable: Disposable { public init() {}; public func set(_ value: Disposable?) {}; public func dispose() {} }
public final class Queue {
    public init() {}
    public func async(_ f: @escaping () -> Void) { f() }
    public func isCurrent() -> Bool { true }
}
public final class QueueLocalObject<T> {
    let value: T
    public init(queue: Queue, generate: () -> T) { value = generate() }
    public func with(_ f: (T) -> Void) { f(value) }
}
public final class Subscriber<T, E: Error> {
    public func putNext(_ value: T) {}
    public func putError(_ value: E) {}
    public func putCompletion() {}
}
public final class Signal<T, E: Error> {
    private var next: ((T) -> Void)?
    private var generator: ((Subscriber<T,E>) -> Disposable)?
    public init() {}
    public init(_ generator: @escaping (Subscriber<T,E>) -> Disposable) { self.generator = generator }
    public func putNext(_ value: T) { next?(value) }
    public func start(next: ((T) -> Void)? = nil, error: ((E) -> Void)? = nil, completed: (() -> Void)? = nil) -> Disposable { self.next = next; return generator?(Subscriber()) ?? EmptyDisposable() }
    public static func single(_ value: T) -> Signal<T, E> { Signal<T, E>() }
}
public final class ValuePromise<T> {
    public init(_ value: T, ignoreRepeated: Bool) {}
    public func set(_ value: T) {}
    public func get() -> Signal<T, NoError> { Signal<T, NoError>() }
}
public final class Timer {
    public static var all: [Timer] = []
    public static var activeCount: Int { all.filter { $0.active }.count }
    private var active = false
    private let completion: () -> Void
    public init(timeout: Double, repeat: Bool, completion: @escaping () -> Void, queue: Queue) { self.completion = completion }
    public func start() { active = true; Timer.all.append(self) }
    public func invalidate() { active = false }
    public static func fireActive() { let current = all.filter { $0.active }; for timer in current { timer.active = false; timer.completion() } }
}
infix operator |>: AdditionPrecedence
public func |> <T, U>(value: T, f: (T) -> U) -> U { f(value) }
public func distinctUntilChanged<T: Equatable, E>(_ signal: Signal<T,E>) -> Signal<T,E> { signal }
public func deliverOn<T,E>(_ queue: Queue) -> (Signal<T,E>) -> Signal<T,E> { { $0 } }
public func `catch`<T,E,F>(_ f: @escaping (E) -> Signal<T,F>) -> (Signal<T,E>) -> Signal<T,F> { { _ in Signal<T,F>() } }
