import Foundation
import CryptoKit
import SystemConfiguration
import Darwin

public enum SlopogramExternalConfiguration {
    private static let loaded: (values: [String: String], error: String?) = {
        let env = ProcessInfo.processInfo.environment
        let explicit = env["SLOPOGRAM_CONFIG_FILE"]
        let executable = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
        let repository = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let candidates = explicit.map { [URL(fileURLWithPath: $0)] } ?? [URL(fileURLWithPath: FileManager.default.currentDirectoryPath).appendingPathComponent(".env"), executable.deletingLastPathComponent().appendingPathComponent(".env"), executable.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent(".env"), repository.appendingPathComponent(".env")]
        var values: [String: String] = [:]
        if let file = candidates.first(where: { explicit != nil || FileManager.default.fileExists(atPath: $0.path) }) {
            guard let text = try? String(contentsOf: file, encoding: .utf8) else { return (env, "Could not read the configuration file at \(file.path).") }
            for (index, raw) in text.components(separatedBy: .newlines).enumerated() {
                var line = raw.trimmingCharacters(in: .whitespaces)
                if line.isEmpty || line.hasPrefix("#") { continue }
                if line.hasPrefix("export ") { line.removeFirst(7) }
                guard let separator = line.firstIndex(of: "=") else { return (env, "Invalid configuration on line \(index + 1).") }
                let key = String(line[..<separator]).trimmingCharacters(in: .whitespaces)
                guard !key.isEmpty, key.allSatisfy({ $0.isLetter || $0.isNumber || $0 == "_" }) else { return (env, "Invalid configuration on line \(index + 1).") }
                var value = String(line[line.index(after: separator)...]).trimmingCharacters(in: .whitespaces)
                if value.hasPrefix("\"") || value.hasPrefix("'") {
                    guard value.count >= 2, value.last == value.first else { return (env, "Invalid configuration on line \(index + 1).") }
                    value = String(value.dropFirst().dropLast())
                } else if let comment = value.range(of: " #") {
                    value = String(value[..<comment.lowerBound]).trimmingCharacters(in: .whitespaces)
                }
                values[key] = value
            }
        }
        return (values.merging(env) { _, environment in environment }, nil)
    }()
    public static var values: [String: String] { loaded.values }
    private static let dataResult: (url: URL?, error: String?) = {
        let selected: URL
        if let path = values["SLOPOGRAM_DATA_DIR"], !path.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            selected = URL(fileURLWithPath: path, isDirectory: true).standardizedFileURL
        } else {
            guard let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first else {
                return (nil, "Could not locate the application data directory.")
            }
            selected = base.appendingPathComponent("Slopogram Native", isDirectory: true)
        }
        var status = stat()
        if lstat(selected.path, &status) != 0 {
            guard errno == ENOENT else { return (nil, "Could not inspect the Slopogram data directory.") }
            do {
                try FileManager.default.createDirectory(at: selected, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            } catch { return (nil, "Could not create the Slopogram data directory.") }
            guard lstat(selected.path, &status) == 0 else { return (nil, "Could not inspect the Slopogram data directory.") }
        }
        guard (status.st_mode & S_IFMT) == S_IFDIR, status.st_uid == getuid(), (status.st_mode & 0o077) == 0, (status.st_mode & 0o700) == 0o700 else {
            return (nil, "The Slopogram data directory must be a private directory owned by the current user, not a symbolic link.")
        }
        return (selected, nil)
    }()
    public static var dataDirectory: URL? { dataResult.url }
    public static func profileKeychainScope(for directory: URL) -> String {
        let canonicalPath = directory.standardizedFileURL.resolvingSymlinksInPath().path
        let digest = SHA256.hash(data: Data(canonicalPath.utf8))
        return "slopogram-profile-" + digest.map { String(format: "%02x", $0) }.joined()
    }
    public static var keychainScope: String? { dataDirectory.map(profileKeychainScope(for:)) }
    public static var biometricKeychainService: String? {
        keychainScope.map { "one.slopogram.native.wallet.biometric." + $0 }
    }
    public struct Proxy {
        public let host: String
        public let port: Int32
        public let username: String?
        public let password: String?
    }
    private static let proxyResult: (proxy: Proxy?, error: String?) = {
        if let explicit = values["SLOPOGRAM_PROXY"]?.trimmingCharacters(in: .whitespacesAndNewlines), !explicit.isEmpty {
            guard let url = URLComponents(string: explicit), url.scheme?.lowercased() == "socks5",
                  let host = url.host, !host.isEmpty, !host.contains(where: { $0.isWhitespace }), let port = url.port, (1...65535).contains(port),
                  (url.path.isEmpty || url.path == "/"), url.query == nil, url.fragment == nil else {
                return (nil, "Invalid SOCKS5 proxy configuration. Check the proxy address and port.")
            }
            return (Proxy(host: host, port: Int32(port), username: url.user, password: url.password), nil)
        }
        let settings = SCDynamicStoreCopyProxies(nil) as? [String: Any] ?? [:]
        guard (settings["SOCKSEnable"] as? NSNumber)?.intValue == 1 else { return (nil, nil) }
        guard let host = settings["SOCKSProxy"] as? String, !host.isEmpty, !host.contains(where: { $0.isWhitespace }),
              let port = (settings["SOCKSPort"] as? NSNumber)?.intValue, (1...65535).contains(port) else {
            return (nil, "Invalid system SOCKS5 proxy configuration. Check the proxy address and port.")
        }
        return (Proxy(host: host, port: Int32(port), username: settings["SOCKSUser"] as? String, password: nil), nil)
    }()
    public static var proxy: Proxy? { proxyResult.proxy }
    public static var error: String? { loaded.error ?? proxyResult.error ?? dataResult.error }
    public static var valid: Bool {
        guard error == nil, let raw = values["TELEGRAM_API_ID"], let id = Int32(raw), id > 0, let hash = values["TELEGRAM_API_HASH"] else { return false }
        return !hash.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }
}
