// swift-tools-version: 6.2
import PackageDescription
import Foundation

let libraryDir = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../target/release").standardizedFileURL.path
let package = Package(
    name: "SlopogramAyu",
    platforms: [.macOS(.v15)],
    products: [.library(name: "SlopogramAyu", targets: ["SlopogramAyu"])],
    targets: [
        .systemLibrary(name: "slopogram_ayu_ffiFFI", path: "Generated/FFI"),
        .target(name: "SlopogramAyu", dependencies: ["slopogram_ayu_ffiFFI"], path: "Generated/Swift", linkerSettings: [.unsafeFlags(["-L", libraryDir, "-lslopogram_ayu_ffi", "-Xlinker", "-rpath", "-Xlinker", "@executable_path/../Frameworks"])]),
        .testTarget(name: "SlopogramAyuTests", dependencies: ["SlopogramAyu"], linkerSettings: [.unsafeFlags(["-Xlinker", "-rpath", "-Xlinker", libraryDir])]),
    ]
)
