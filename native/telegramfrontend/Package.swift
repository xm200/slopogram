// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "SlopogramTelegramFrontend",
    platforms: [.macOS(.v15)],
    products: [.executable(name: "SlopogramTelegram", targets: ["SlopogramTelegram"])],
    dependencies: [
        .package(name: "TGUIKit", path: "../telegram-macos/packages/TGUIKit"),
        .package(name: "ColorPalette", path: "../telegram-macos/packages/ColorPalette"),
        .package(name: "SlopogramNative", path: "../macos"),
    ],
    targets: [
        .executableTarget(name: "SlopogramTelegram", dependencies: [
            .product(name: "TGUIKit", package: "TGUIKit"),
            .product(name: "ColorPalette", package: "ColorPalette"),
            .product(name: "RustBridge", package: "SlopogramNative"),
            .product(name: "SlopogramConfig", package: "SlopogramNative"),
            .product(name: "SlopogramState", package: "SlopogramNative"),
        ]),
        .testTarget(name: "SlopogramTelegramTests", dependencies: ["SlopogramTelegram"]),
    ]
)
