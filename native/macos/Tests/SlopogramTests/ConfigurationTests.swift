import Foundation
import XCTest
import SlopogramConfig

final class ConfigurationTests: XCTestCase {
    func testCandidateOrderAndParsing() throws {
        let paths = SlopogramConfiguration.candidates(cwd: URL(fileURLWithPath: "/work"), executable: URL(fileURLWithPath: "/app/bin"), repository: URL(fileURLWithPath: "/repo"))
        XCTAssertEqual(paths.map(\.path), ["/work/.env", "/app/.env", "/repo/.env"])
        XCTAssertEqual(try SlopogramConfiguration.parse("# comment\nexport API_ID=123\nNAME='hello'"), ["API_ID": "123", "NAME": "hello"])
        XCTAssertThrowsError(try SlopogramConfiguration.parse("invalid"))
        XCTAssertEqual(try SlopogramConfiguration.parse("A=abc # comment\nB=\"#value\""), ["A": "abc", "B": "#value"])
    }
    func testConfigurationErrorsAreActionable() {
        XCTAssertTrue(ConfigurationError.invalidProxy.localizedDescription.contains("SOCKS5"))
        XCTAssertTrue(ConfigurationError.invalidLine(42).localizedDescription.contains("42"))
    }
    func testProxyPrecedenceAndValidation() throws {
        let system: [String: Any] = ["SOCKSEnable": 1, "SOCKSProxy": "127.0.0.1", "SOCKSPort": 1080]
        XCTAssertEqual(try SlopogramConfiguration.proxy(environment: [:], system: system), "socks5://127.0.0.1:1080")
        XCTAssertEqual(try SlopogramConfiguration.proxy(environment: ["SLOPOGRAM_PROXY": "socks5://localhost:9050"], system: system), "socks5://localhost:9050")
        XCTAssertThrowsError(try SlopogramConfiguration.proxy(environment: ["SLOPOGRAM_PROXY": "invalid"], system: system))
        XCTAssertEqual(try SlopogramConfiguration.proxy(environment: ["SLOPOGRAM_PROXY": "   "], system: system), "socks5://127.0.0.1:1080")
        XCTAssertEqual(try SlopogramConfiguration.proxy(environment: ["SLOPOGRAM_PROXY": " socks5://localhost:9050 "], system: system), "socks5://localhost:9050")
    }
}
