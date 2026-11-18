import AppKit
import XCTest

@testable import SlopogramAyu

final class ThemeEditorTests: XCTestCase {
  func testHexNormalizationAndValidation() {
    XCTAssertEqual(normalizedDesktopThemeHex("  aabbcc  "), "#AABBCC")
    XCTAssertEqual(normalizedDesktopThemeHex("#12345678"), "#12345678")
    XCTAssertNil(normalizedDesktopThemeHex("##123456"))
    XCTAssertNil(normalizedDesktopThemeHex("#12345"))
    XCTAssertNil(normalizedDesktopThemeHex("#12345g"))
  }
}

extension ThemeEditorTests {
  func testWallpaperAndEncodedColorsRoundTrip() async throws {
    let bytes = Data(
      base64Encoded:
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII="
    )!
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(
      UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let input = root.appendingPathComponent("wallpaper.png")
    try bytes.write(to: input)
    let (wallpaper, ext, video) = try await prepareEditorWallpaper(url: input)
    XCTAssertEqual(wallpaper, bytes)
    XCTAssertEqual(ext, "png")
    XCTAssertNil(video)
    let archive = try await encodeDesktopTheme(
      colors: [DesktopThemeColor(key: "windowBg", value: "#123456FF")], wallpaper: wallpaper,
      wallpaperExtension: ext, wallpaperTiled: false, video: nil)
    let output = root.appendingPathComponent("theme.tdesktop-theme")
    try archive.write(to: output)
    let decoded = try await decodeDesktopTheme(path: output.path)
    XCTAssertTrue(decoded.palette.lowercased().contains("#123456"))
    XCTAssertEqual(decoded.wallpaper, bytes)
  }

  @MainActor func testSyntheticEditorScreenshot() async throws {
    guard let path = ProcessInfo.processInfo.environment["SLOPOGRAM_EDITOR_SCREENSHOT"] else {
      throw XCTSkip("Set SLOPOGRAM_EDITOR_SCREENSHOT to capture the synthetic editor.")
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let keys = [
      "background", "basicAccent", "text", "grayText", "link", "accent", "redUI", "greenUI",
      "blackTransparent", "grayTransparent", "grayUI", "darkGrayText", "accentSelect", "selectText",
      "border", "grayBackground", "grayForeground", "grayIcon", "accentIcon", "badgeMuted", "badge",
      "indicatorColor", "selectMessage", "monospacedPre", "monospacedCode",
      "selectTextBubble_incoming", "selectTextBubble_outgoing", "bubbleBackground_incoming",
      "bubbleBackgroundTop_outgoing", "bubbleBorder_incoming", "bubbleBorder_outgoing",
      "grayTextBubble_incoming", "grayTextBubble_outgoing", "grayIconBubble_incoming",
      "grayIconBubble_outgoing", "accentIconBubble_incoming", "accentIconBubble_outgoing",
      "linkBubble_incoming", "linkBubble_outgoing", "textBubble_incoming", "textBubble_outgoing",
      "fileActivityBackground", "fileActivityForeground",
    ]
    let palette = keys.enumerated().map { index, key in
      "\(key) = " + (index % 2 == 0 ? "#daebf5" : "#214765")
    }.joined(separator: "\n")
    let bytes = Data(
      base64Encoded:
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII="
    )!
    let editor = SlopogramThemeEditorController(
      palette: palette, wallpaper: bytes, wallpaperExtension: "png", wallpaperTiled: false,
      video: nil
    ) { _, _ in }
    editor.showWindow(nil)
    editor.window?.center()
    editor.window?.makeKeyAndOrderFront(nil)
    app.activate(ignoringOtherApps: true)
    try await Task.sleep(for: .seconds(1))
    guard let view = editor.window?.contentView,
      let bitmap = view.bitmapImageRepForCachingDisplay(in: view.bounds)
    else {
      XCTFail("Cannot capture editor")
      return
    }
    view.layoutSubtreeIfNeeded()
    view.cacheDisplay(in: view.bounds, to: bitmap)
    guard let png = bitmap.representation(using: .png, properties: [:]) else {
      XCTFail("Cannot encode screenshot")
      return
    }
    try png.write(to: URL(fileURLWithPath: path))
    editor.close()
  }
}

extension ThemeEditorTests {
  @MainActor func testEditorControlsAndApplyArchive() async throws {
    guard ProcessInfo.processInfo.environment["SLOPOGRAM_EDITOR_GUI_TEST"] == "1" else {
      throw XCTSkip("Set SLOPOGRAM_EDITOR_GUI_TEST=1 for the synthetic AppKit fixture.")
    }
    _ = NSApplication.shared
    let wallpaper = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=")!
    var applied: Data?
    let editor = SlopogramThemeEditorController(
      palette: "background = #daebf5\ntext = #214765", wallpaper: wallpaper,
      wallpaperExtension: "png", wallpaperTiled: false, video: nil
    ) { data, _ in applied = data }
    editor.showWindow(nil)
    defer { editor.close() }
    func descendants(_ view: NSView) -> [NSView] {
      [view] + view.subviews.flatMap { descendants($0) }
    }
    let root = try XCTUnwrap(editor.window?.contentView)
    var field: NSTextField?
    for _ in 0..<100 {
      field = descendants(root).compactMap { $0 as? NSTextField }.first {
        $0.action == NSSelectorFromString("hexChanged:")
      }
      if field != nil { break }
      try await Task.sleep(for: .milliseconds(20))
    }
    let hex = try XCTUnwrap(field)
    let tag = hex.tag
    hex.stringValue = "#12345678"
    _ = NSApp.sendAction(try XCTUnwrap(hex.action), to: hex.target, from: hex)
    let well = try XCTUnwrap(descendants(root).compactMap { $0 as? NSColorWell }.first { $0.tag == tag })
    let color = try XCTUnwrap(well.color.usingColorSpace(.sRGB))
    XCTAssertEqual(color.redComponent, 0x12 / 255.0, accuracy: 0.005)
    XCTAssertEqual(color.alphaComponent, 0x78 / 255.0, accuracy: 0.005)
    hex.stringValue = "bad"
    _ = NSApp.sendAction(try XCTUnwrap(hex.action), to: hex.target, from: hex)
    let search = try XCTUnwrap(descendants(root).compactMap { $0 as? NSSearchField }.first)
    search.stringValue = "no such color"
    _ = NSApp.sendAction(try XCTUnwrap(search.action), to: search.target, from: search)
    let apply = try XCTUnwrap(descendants(root).compactMap { $0 as? NSButton }.first { $0.title == "Apply" })
    _ = NSApp.sendAction(try XCTUnwrap(apply.action), to: apply.target, from: apply)
    XCTAssertNil(applied)
    search.stringValue = ""
    _ = NSApp.sendAction(try XCTUnwrap(search.action), to: search.target, from: search)
    let corrected = try XCTUnwrap(descendants(root).compactMap { $0 as? NSTextField }.first {
      $0.action == NSSelectorFromString("hexChanged:") && $0.tag == tag
    })
    corrected.stringValue = "#12345678"
    _ = NSApp.sendAction(try XCTUnwrap(corrected.action), to: corrected.target, from: corrected)
    _ = NSApp.sendAction(try XCTUnwrap(apply.action), to: apply.target, from: apply)
    for _ in 0..<100 {
      if applied != nil { break }
      try await Task.sleep(for: .milliseconds(20))
    }
    let archive = try XCTUnwrap(applied)
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".tdesktop-theme")
    defer { try? FileManager.default.removeItem(at: url) }
    try archive.write(to: url)
    let decoded = try await decodeDesktopTheme(path: url.path)
    XCTAssertTrue(decoded.palette.lowercased().contains("#123456"))
    XCTAssertEqual(decoded.wallpaper, wallpaper)
  }
}
