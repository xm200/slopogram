import Foundation

@main struct GhostPolicyNotificationTest {
    static func main() {
        var first = 0
        var second = 0
        let a = SlopogramGhostPolicy.observe(path: "account-a") { first += 1 }
        let b = SlopogramGhostPolicy.observe(path: "account-b") { second += 1 }
        SlopogramGhostPolicy.notifyPolicyChanged(path: "account-a")
        precondition(first == 1 && second == 0)
        SlopogramGhostPolicy.notifyPolicyChanged(path: "account-b")
        precondition(first == 1 && second == 1)
        SlopogramGhostPolicy.removeObserver(a)
        SlopogramGhostPolicy.notifyPolicyChanged(path: "account-a")
        precondition(first == 1 && second == 1)
        SlopogramGhostPolicy.removeObserver(b)
    }
}
