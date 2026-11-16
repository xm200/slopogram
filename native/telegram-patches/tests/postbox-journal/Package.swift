// swift-tools-version: 5.5
import PackageDescription
import Foundation
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardizedFileURL
let ios = root.appendingPathComponent("native/telegram-macos/submodules/telegram-ios/submodules")
let package = Package(name: "PostboxJournalTests", platforms: [.macOS(.v12)], dependencies: [
    .package(name: "Postbox", path: ios.appendingPathComponent("Postbox").path),
    .package(name: "SSignalKit", path: ios.appendingPathComponent("SSignalKit").path),
], targets: [.testTarget(name: "PostboxJournalTests", dependencies: ["Postbox", .product(name: "SwiftSignalKit", package: "SSignalKit")])])
