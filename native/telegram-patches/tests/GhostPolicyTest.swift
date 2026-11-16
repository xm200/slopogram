import Foundation

@main struct GhostPolicyTest {
    static func main() {
        precondition(!SlopogramGhostPolicy.isSuppressed(accountBasePath: "first", control: .typing))
        SlopogramGhostPolicy.installProvider { path, control in path == "first" && control == .typing }
        precondition(SlopogramGhostPolicy.isSuppressed(accountBasePath: "first", control: .typing))
        precondition(!SlopogramGhostPolicy.isSuppressed(accountBasePath: "first", control: .online))
        precondition(!SlopogramGhostPolicy.isSuppressed(accountBasePath: "second", control: .typing))
        SlopogramGhostPolicy.installProvider(nil)
        precondition(!SlopogramGhostPolicy.isSuppressed(accountBasePath: "first", control: .typing))
    }
}
