import XCTest
@preconcurrency import AVFoundation
@testable import SlopogramAyu

final class VideoThemeSourceTests: XCTestCase {
    func testPortraitHighFrameRateIsNormalized() async throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let input = directory.appendingPathComponent("input.mp4")
        // One second of synthetic red portrait video, H.264 at 120 fps.
        let fixture = try XCTUnwrap(Bundle.module.url(forResource: "high-fps-portrait", withExtension: "mp4", subdirectory: "Fixtures"))
        try FileManager.default.copyItem(at: fixture, to: input)
        let result = try await prepareVideoThemeSource(url: input)
        XCTAssertLessThanOrEqual(result.video.count, 16 * 1024 * 1024)
        XCTAssertFalse(result.poster.isEmpty)
        XCTAssertEqual(result.samples.count, 4 * 16 * 16 * 3)
        let output = directory.appendingPathComponent("output.mp4")
        try result.video.write(to: output)
        let asset = AVURLAsset(url: output)
        let tracks = try await asset.loadTracks(withMediaType: .video)
        let track = try XCTUnwrap(tracks.first)
        let fps = try await track.load(.nominalFrameRate)
        let size = try await track.load(.naturalSize)
        let audio = try await asset.loadTracks(withMediaType: .audio)
        XCTAssertLessThanOrEqual(fps, 60)
        XCTAssertLessThan(size.width, size.height)
        XCTAssertTrue(audio.isEmpty)
        let compatible = try await prepareVideoThemeSource(url: output)
        XCTAssertEqual(compatible.video, result.video)
    }
}
