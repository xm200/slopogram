#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/Sources/Fixture"
cp "$root/native/telegram-macos/submodules/telegram-ios/submodules/TelegramCore/Sources/State/MessageDeletionEventPipe.swift" "$scratch/Sources/Fixture/"
cat > "$scratch/Package.swift" <<'SWIFT'
// swift-tools-version: 6.2
import Foundation
import PackageDescription
let root = ProcessInfo.processInfo.environment["SLOPOGRAM_SOURCE_ROOT"]!
let package = Package(name: "DeletionFixture", platforms: [.macOS(.v15)], dependencies: [.package(name: "SSignalKit", path: root + "/native/telegram-macos/submodules/telegram-ios/submodules/SSignalKit")], targets: [.executableTarget(name: "Fixture", dependencies: [.product(name: "SwiftSignalKit", package: "SSignalKit")])])
SWIFT
cat > "$scratch/Sources/Fixture/main.swift" <<'SWIFT'
import SwiftSignalKit

enum DeletedMessageId: Hashable { case global(Int32); case messageId(Int32) }
enum MessageDeletionOrigin { case interactiveLocal; case remoteConfirmed }
struct MessageDeletionEvent { let ids: [DeletedMessageId]; let origin: MessageDeletionOrigin }
let pipe = MessageDeletionEventPipe()
var actual: [MessageDeletionEvent] = []
let subscription = pipe.events.start(next: { actual.append($0) })
pipe.interactive([.messageId(5)])
pipe.confirmed([.global(5), .messageId(9)])
pipe.confirmed([])
precondition(actual.count == 2)
precondition(actual[0].origin == .interactiveLocal && actual[0].ids == [.messageId(5)])
precondition(actual[1].origin == .remoteConfirmed && actual[1].ids == [.global(5), .messageId(9)])
subscription.dispose()
SWIFT
SLOPOGRAM_SOURCE_ROOT="$root" swift run ${SLOPOGRAM_SWIFT_BUILD_FLAGS:-} --package-path "$scratch" Fixture
