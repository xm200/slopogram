import AppKit
import ImageIO
import UniformTypeIdentifiers

let arguments = CommandLine.arguments
precondition(arguments.count == 4, "Usage: split-macos-icon.swift input.png plane.png knot.png")
let input = URL(fileURLWithPath: arguments[1]) as CFURL
guard let source = CGImageSourceCreateWithURL(input, nil), let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { fatalError("Cannot read icon foreground") }
let width = image.width, height = image.height, rowBytes = width * 4
var pixels = [UInt8](repeating: 0, count: rowBytes * height)
guard let context = CGContext(data: &pixels, width: width, height: height, bitsPerComponent: 8, bytesPerRow: rowBytes, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue) else { fatalError("Cannot create bitmap") }
context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
var plane = [UInt8](repeating: 0, count: pixels.count)
var knot = plane
for offset in stride(from: 0, to: pixels.count, by: 4) {
    let alpha = Int(pixels[offset + 3])
    guard alpha > 0 else { continue }
    let brightness = (Int(pixels[offset]) + Int(pixels[offset + 1]) + Int(pixels[offset + 2])) * 255 / (3 * alpha)
    if brightness < 115 {
        plane[offset] = pixels[offset + 3]
        plane[offset + 1] = pixels[offset + 3]
        plane[offset + 2] = pixels[offset + 3]
        plane[offset + 3] = pixels[offset + 3]
    } else {
        for component in 0..<4 { knot[offset + component] = pixels[offset + component] }
    }
}
func save(_ data: inout [UInt8], _ path: String) {
    guard let context = CGContext(data: &data, width: width, height: height, bitsPerComponent: 8, bytesPerRow: rowBytes, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue), let result = context.makeImage(), let output = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL, UTType.png.identifier as CFString, 1, nil) else { fatalError("Cannot create icon layer") }
    CGImageDestinationAddImage(output, result, nil)
    guard CGImageDestinationFinalize(output) else { fatalError("Cannot write icon layer") }
}
save(&plane, arguments[2])
save(&knot, arguments[3])
