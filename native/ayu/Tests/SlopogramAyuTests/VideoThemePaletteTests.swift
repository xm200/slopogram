import Foundation
import XCTest
@testable import SlopogramAyu

final class VideoThemePaletteTests: XCTestCase {
    func testSceneChangesPaletteAndAppearance() {
        let dark = makeVideoThemePalette(name: "Aurora\nparent = dayClassic", samples: Array(repeating: [UInt8(12), 62, 55], count: 20).flatMap { $0 })
        let light = makeVideoThemePalette(name: "Sun", samples: Array(repeating: [UInt8(245), 210, 160], count: 20).flatMap { $0 })
        XCTAssertTrue(dark.contains("isDark = 1\n"))
        XCTAssertTrue(light.contains("isDark = 0\n"))
        XCTAssertFalse(dark.contains("\nparent = dayClassic\n"))
        XCTAssertNotEqual(dark, light)
        XCTAssertTrue(dark.contains("bubbleBackground_incoming = #"))
        XCTAssertTrue(dark.contains("textBubble_outgoing = #"))
        XCTAssertTrue(dark.contains("grayTextBubble_incoming = #"))
    }
    func testEmptyAndIncompleteSamplesAreSafe() {
        XCTAssertTrue(makeVideoThemePalette(name: "", samples: [0, 1]).contains("name = Video Theme\n"))
    }
}

extension VideoThemePaletteTests {
    func testSparseWarmAccentAndContrast() {
        let samples: [UInt8] = Array(repeating: [UInt8(0), 0, 0], count: 100).flatMap { $0 } + [UInt8(255), 60, 10, 250, 40, 5]
        let palette = makeVideoThemePalette(name: "Nebula", samples: samples)
        let values: [String: String] = Dictionary(palette.split(separator: "\n").compactMap { line -> (String, String)? in
            let parts = String(line).components(separatedBy: " = ")
            return parts.count == 2 ? (parts[0], parts[1]) : nil
        }, uniquingKeysWith: { _, last in last })
        XCTAssertNotEqual(values["accent"], "#bebebe")
        func luminance(_ value: String) -> Double {
            let hex = UInt32(value.dropFirst(), radix: 16)!
            let raw: [UInt32] = [hex >> 16, (hex >> 8) & 255, hex & 255]
            let channels: [Double] = raw.map { channel in
                let v = Double(channel) / 255.0
                return v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2]
        }
        for foreground in ["textBubble_incoming", "linkBubble_incoming"] {
            for background in ["bubbleBackground_incoming", "bubbleBackground_outgoing", "selectTextBubble_incoming"] {
                let a = luminance(values[foreground]!), b = luminance(values[background]!)
                XCTAssertGreaterThan((max(a, b) + 0.05) / (min(a, b) + 0.05), 4.5)
            }
        }
    }
}
