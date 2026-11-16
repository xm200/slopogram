import Foundation
internal import TelegramApi
import SwiftSignalKit
import MtProtoKit

final class Network {
    var calls: [Api.Bool] = []
    func request(_ value: Api.Bool) -> Signal<Api.Bool, MTRpcError> { calls.append(value); return Signal() }
}

@main struct PresenceTest {
    static func main() {
        var suppressed = true
        SlopogramGhostPolicy.installProvider { path, control in path == "a" && control == .online && suppressed }
        let online = Signal<Bool, NoError>()
        let network = Network()
        let manager = AccountPresenceManager(shouldKeepOnlinePresence: online, network: network, accountBasePath: "a")
        online.putNext(true)
        precondition(Timer.activeCount == 0 && network.calls.isEmpty)
        for _ in 0..<10 { Timer.fireActive() }
        precondition(Timer.activeCount == 0 && network.calls.isEmpty)
        suppressed = false
        SlopogramGhostPolicy.notifyPolicyChanged(path: "b")
        precondition(Timer.activeCount == 0)
        SlopogramGhostPolicy.notifyPolicyChanged(path: "a")
        precondition(Timer.activeCount == 1 && network.calls.count == 1)
        Timer.fireActive()
        precondition(Timer.activeCount == 1 && network.calls.count == 2)
        suppressed = true
        SlopogramGhostPolicy.notifyPolicyChanged(path: "a")
        precondition(Timer.activeCount == 0)
        Timer.fireActive()
        precondition(network.calls.count == 2)
        suppressed = false
        SlopogramGhostPolicy.notifyPolicyChanged(path: "a")
        precondition(Timer.activeCount == 1 && network.calls.count == 3)
        online.putNext(false)
        precondition(Timer.activeCount == 0 && network.calls.count == 4)
        withExtendedLifetime(manager) {}
    }
}
