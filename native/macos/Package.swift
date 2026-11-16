// swift-tools-version: 6.2
import PackageDescription
import Foundation

let archive = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../target/release/libslopogram_ffi.a").standardizedFileURL.path

let package = Package(
    name: "SlopogramNative",
    platforms: [.macOS(.v15)],
    products: [
        .executable(name: "Slopogram", targets: ["Slopogram"]),
        .library(name: "RustBridge", targets: ["RustBridge"]),
        .library(name: "SlopogramConfig", targets: ["SlopogramConfig"]),
        .library(name: "SlopogramState", targets: ["SlopogramState"]),
    ],
    targets: [
        .systemLibrary(name: "slopogram_ffiFFI", path: "Generated/FFI"),
        .target(name: "RustBridge", dependencies: ["slopogram_ffiFFI"], path: "Generated/Swift", linkerSettings: [.unsafeFlags([archive])]),
        .target(name: "SlopogramConfig", linkerSettings: [.linkedFramework("SystemConfiguration")]),
        .target(name: "SlopogramState", dependencies: ["RustBridge"]),
        .executableTarget(name: "Slopogram", dependencies: ["RustBridge", "SlopogramConfig", "SlopogramState"]),
        .testTarget(name: "SlopogramTests", dependencies: ["Slopogram", "SlopogramConfig", "SlopogramState"]),
    ]
)
