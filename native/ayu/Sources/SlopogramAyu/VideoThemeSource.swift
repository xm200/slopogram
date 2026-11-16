import Foundation
@preconcurrency import AVFoundation
import AppKit

public enum VideoThemeSourceError: LocalizedError {
    case invalidSource, noVideo, exportFailed(String), posterFailed, tooLarge, sourceTooLarge
    public var errorDescription: String? {
        switch self {
        case .invalidSource: return "Choose a local MP4 video."
        case .noVideo: return "The file does not contain a usable video track."
        case .exportFailed(let reason): return "Could not prepare the video: \(reason)"
        case .posterFailed: return "Could not generate a preview image."
        case .tooLarge: return "The prepared video exceeds 16 MiB."
        case .sourceTooLarge: return "The source video exceeds 512 MiB."
        }
    }
}

/// Prepares a silent, bounded H.264 loop and color samples. Call from a detached task.
public func prepareVideoThemeSource(url: URL) async throws -> (video: Data, poster: Data, samples: [UInt8]) {
    guard url.isFileURL, url.pathExtension.lowercased() == "mp4" else { throw VideoThemeSourceError.invalidSource }
    let resolved = url.resolvingSymlinksInPath()
    let attributes = try FileManager.default.attributesOfItem(atPath: resolved.path)
    guard attributes[.type] as? FileAttributeType == .typeRegular else { throw VideoThemeSourceError.invalidSource }
    let inputSize = (attributes[.size] as? NSNumber)?.intValue ?? 0
    guard inputSize > 0 else { throw VideoThemeSourceError.invalidSource }
    guard inputSize <= 512 * 1024 * 1024 else { throw VideoThemeSourceError.sourceTooLarge }
    let scratch = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory: true)
    try FileManager.default.createDirectory(at: scratch, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
    defer { try? FileManager.default.removeItem(at: scratch) }
    let snapshot = scratch.appendingPathComponent("source.mp4")
    try FileManager.default.copyItem(at: resolved, to: snapshot)
    let sourceSize = (try FileManager.default.attributesOfItem(atPath: snapshot.path)[.size] as? NSNumber)?.intValue ?? 0
    guard sourceSize <= 512 * 1024 * 1024 else { throw VideoThemeSourceError.sourceTooLarge }
    let asset = AVURLAsset(url: snapshot, options: [AVURLAssetPreferPreciseDurationAndTimingKey: true, AVURLAssetReferenceRestrictionsKey: AVAssetReferenceRestrictions.forbidAll.rawValue])
    let tracks = try await asset.loadTracks(withMediaType: .video)
    guard let track = tracks.first else { throw VideoThemeSourceError.noVideo }
    let videoRange = try await track.load(.timeRange)
    let duration = videoRange.duration
    guard duration.isNumeric, duration.seconds.isFinite, duration.seconds > 0 else { throw VideoThemeSourceError.noVideo }
    let assetDuration = try await asset.load(.duration)
    let descriptions = try await track.load(.formatDescriptions)
    let dimensions = try await track.load(.naturalSize)
    let audio = try await asset.loadTracks(withMediaType: .audio)
    let fps = try await track.load(.nominalFrameRate)
    let h264 = !descriptions.isEmpty && descriptions.allSatisfy { CMFormatDescriptionGetMediaSubType($0) == kCMVideoCodecType_H264 }
    let compatible = tracks.count == 1 && audio.isEmpty && h264 && fps > 0 && fps <= 60
        && assetDuration.seconds > 0 && assetDuration.seconds <= 30
        && dimensions.width > 0 && dimensions.height > 0 && max(dimensions.width, dimensions.height) <= 1920
        && sourceSize > 0 && sourceSize <= 16 * 1024 * 1024
    let temporary = scratch.appendingPathComponent("prepared.mp4")
    let preparedURL: URL
    let length = CMTimeMinimum(duration, CMTime(seconds: 15, preferredTimescale: 600))
    if compatible {
        preparedURL = snapshot
    } else {
        // A composition strips audio and bounds the loop before encoding.
        let composition = AVMutableComposition()
        guard let output = composition.addMutableTrack(withMediaType: .video, preferredTrackID: kCMPersistentTrackID_Invalid) else { throw VideoThemeSourceError.noVideo }
        try output.insertTimeRange(CMTimeRange(start: videoRange.start, duration: length), of: track, at: .zero)
        output.preferredTransform = try await track.load(.preferredTransform)
        guard let exporter = AVAssetExportSession(asset: composition, presetName: AVAssetExportPreset1280x720) else { throw VideoThemeSourceError.exportFailed("Unsupported video format") }
        exporter.shouldOptimizeForNetworkUse = true
        exporter.fileLengthLimit = 16 * 1024 * 1024
        if fps > 60 {
            let videoComposition = AVMutableVideoComposition(propertiesOf: composition)
            videoComposition.sourceTrackIDForFrameTiming = kCMPersistentTrackID_Invalid
            videoComposition.frameDuration = CMTime(value: 1, timescale: 60)
            exporter.videoComposition = videoComposition
        }
        do { try await exporter.export(to: temporary, as: .mp4) }
        catch { throw VideoThemeSourceError.exportFailed(error.localizedDescription) }
        let resultAsset = AVURLAsset(url: temporary, options: [AVURLAssetReferenceRestrictionsKey: AVAssetReferenceRestrictions.forbidAll.rawValue])
        guard let resultTrack = try await resultAsset.loadTracks(withMediaType: .video).first,
              try await resultTrack.load(.nominalFrameRate) <= 60 else {
            throw VideoThemeSourceError.exportFailed("The video frame rate could not be reduced to 60 fps.")
        }
        preparedURL = temporary
    }
    let size = (try FileManager.default.attributesOfItem(atPath: preparedURL.path)[.size] as? NSNumber)?.intValue ?? 0
    guard size > 0, size <= 16 * 1024 * 1024 else { throw VideoThemeSourceError.tooLarge }
    let video = try Data(contentsOf: preparedURL, options: .mappedIfSafe)
    let generator = AVAssetImageGenerator(asset: AVURLAsset(url: preparedURL, options: [AVURLAssetReferenceRestrictionsKey: AVAssetReferenceRestrictions.forbidAll.rawValue]))
    generator.appliesPreferredTrackTransform = true
    // Prefer the first playable frame, allowing a short tolerance for edit lists.
    generator.requestedTimeToleranceBefore = .zero
    generator.requestedTimeToleranceAfter = CMTime(seconds: 0.25, preferredTimescale: 600)
    generator.maximumSize = CGSize(width: 1280, height: 1280)
    let start = compatible ? videoRange.start : CMTime.zero
    let first = try await generator.image(at: start).image
    let bitmap = NSBitmapImageRep(cgImage: first)
    guard let poster = bitmap.representation(using: .jpeg, properties: [.compressionFactor: 0.8]) else { throw VideoThemeSourceError.posterFailed }
    generator.maximumSize = CGSize(width: 32, height: 32)
    generator.requestedTimeToleranceBefore = .positiveInfinity
    generator.requestedTimeToleranceAfter = .positiveInfinity
    var samples: [UInt8] = []
    for fraction in [0.0, 0.25, 0.5, 0.75] {
        let sampleLength = compatible ? duration : length
        let image = fraction == 0 ? first : try await generator.image(at: CMTimeAdd(start, CMTime(seconds: sampleLength.seconds * fraction, preferredTimescale: 600))).image
        var pixels = [UInt8](repeating: 0, count: 16 * 16 * 4)
        let rendered = pixels.withUnsafeMutableBytes { bytes -> Bool in
            guard let context = CGContext(data: bytes.baseAddress, width: 16, height: 16, bitsPerComponent: 8, bytesPerRow: 64, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: 16, height: 16))
            return true
        }
        guard rendered else { throw VideoThemeSourceError.posterFailed }
        for index in stride(from: 0, to: pixels.count, by: 4) { samples.append(contentsOf: pixels[index..<(index + 3)]) }
    }
    return (video, poster, samples)
}
