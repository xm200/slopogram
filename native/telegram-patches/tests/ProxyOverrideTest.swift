import Foundation
public struct ProxyServerSettings: Equatable { let host: String }
@main struct Test {
    static func main() {
        let saved = ProxyServerSettings(host: "saved")
        let external = ProxyServerSettings(host: "external")
        precondition(SlopogramProxyOverride.resolve(saved) == saved)
        SlopogramProxyOverride.set(external)
        precondition(SlopogramProxyOverride.resolve(saved) == external)
        precondition(SlopogramProxyOverride.resolve(nil) == external)
        SlopogramProxyOverride.set(nil)
        precondition(SlopogramProxyOverride.resolve(saved) == saved)
        print("Proxy override check passed")
    }
}
