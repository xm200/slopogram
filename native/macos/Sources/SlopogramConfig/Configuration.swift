import Foundation
import SystemConfiguration

public enum ConfigurationError: LocalizedError {
    case unreadableFile(String)
    case invalidLine(Int)
    case invalidProxy

    public var errorDescription: String? {
        switch self {
        case .unreadableFile(let path):
            return "Could not read the configuration file at \(path). Check that it exists and is readable."
        case .invalidLine(let line):
            return "Invalid configuration on line \(line). Check its name and value."
        case .invalidProxy:
            return "Invalid SOCKS5 proxy configuration. Check the proxy address and port."
        }
    }
}

public enum SlopogramConfiguration {
    public static var developmentRepository: URL {
        URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    }

    public static func candidates(cwd: URL, executable: URL, repository: URL) -> [URL] {
        var paths = [cwd.appendingPathComponent(".env"), executable.deletingLastPathComponent().appendingPathComponent(".env")]
        var parent = executable.deletingLastPathComponent()
        while parent.path != "/" {
            if parent.pathExtension == "app" {
                paths.append(parent.deletingLastPathComponent().appendingPathComponent(".env"))
                break
            }
            parent.deleteLastPathComponent()
        }
        paths.append(repository.appendingPathComponent(".env"))
        return paths
    }

    public static func parse(_ text: String) throws -> [String: String] {
        var values: [String: String] = [:]
        for (index, raw) in text.components(separatedBy: .newlines).enumerated() {
            var line = raw.trimmingCharacters(in: .whitespaces)
            if line.isEmpty || line.hasPrefix("#") { continue }
            if line.hasPrefix("export ") { line.removeFirst(7) }
            guard let equals = line.firstIndex(of: "=") else { throw ConfigurationError.invalidLine(index + 1) }
            let key = String(line[..<equals]).trimmingCharacters(in: .whitespaces)
            var value = String(line[line.index(after: equals)...]).trimmingCharacters(in: .whitespaces)
            guard !key.isEmpty, key.allSatisfy({ $0.isLetter || $0.isNumber || $0 == "_" }) else { throw ConfigurationError.invalidLine(index + 1) }
            if value.hasPrefix("\"") || value.hasPrefix("'") {
                guard value.count >= 2, value.last == value.first else { throw ConfigurationError.invalidLine(index + 1) }
                value = String(value.dropFirst().dropLast())
            } else if let comment = value.range(of: " #") {
                value = String(value[..<comment.lowerBound]).trimmingCharacters(in: .whitespaces)
            }
            values[key] = value
        }
        return values
    }

    public static func load(repository: URL) throws -> [String: String] {
        let environment = ProcessInfo.processInfo.environment
        let explicit = environment["SLOPOGRAM_CONFIG_FILE"].map { URL(fileURLWithPath: $0) }
        let paths = explicit.map { [$0] } ?? candidates(cwd: URL(fileURLWithPath: FileManager.default.currentDirectoryPath), executable: URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL, repository: repository)
        var result: [String: String] = [:]
        if let path = paths.first(where: { explicit != nil || FileManager.default.fileExists(atPath: $0.path) }) {
            guard let content = try? String(contentsOf: path, encoding: .utf8) else { throw ConfigurationError.unreadableFile(path.path) }
            result = try parse(content)
        }
        return result.merging(environment) { _, existing in existing }
    }

    public static func proxy(environment: [String: String], system: [String: Any] = (SCDynamicStoreCopyProxies(nil) as? [String: Any] ?? [:])) throws -> String? {
        if let explicit = environment["SLOPOGRAM_PROXY"]?.trimmingCharacters(in: .whitespacesAndNewlines), !explicit.isEmpty {
            guard let components = URLComponents(string: explicit), components.scheme?.lowercased() == "socks5", components.host?.isEmpty == false, let port = components.port, (1...65535).contains(port) else { throw ConfigurationError.invalidProxy }
            return explicit
        }
        guard (system["SOCKSEnable"] as? NSNumber)?.intValue == 1 else { return nil }
        guard let host = system["SOCKSProxy"] as? String, !host.isEmpty, let port = (system["SOCKSPort"] as? NSNumber)?.intValue, (1...65535).contains(port) else { throw ConfigurationError.invalidProxy }
        var components = URLComponents(); components.scheme = "socks5"; components.host = host; components.port = port
        components.user = system["SOCKSUser"] as? String
        guard let url = components.string else { throw ConfigurationError.invalidProxy }
        return url
    }
}
