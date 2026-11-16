import Foundation

/// Creates a Telegram macOS palette from packed RGB frame samples.
public func makeVideoThemePalette(name: String, samples: [UInt8]) -> String {
    var bins = Array(repeating: (count: 0, r: 0, g: 0, b: 0), count: 512)
    var brightness = 0
    let count = samples.count / 3
    for i in 0..<count {
        let r = Int(samples[i * 3]), g = Int(samples[i * 3 + 1]), b = Int(samples[i * 3 + 2])
        let index = (r >> 5) * 64 + (g >> 5) * 8 + (b >> 5)
        bins[index].count += 1; bins[index].r += r; bins[index].g += g; bins[index].b += b
        brightness += 299 * r + 587 * g + 114 * b
    }
    let dominant = bins.max { $0.count < $1.count }!
    func average(_ bin: (count: Int, r: Int, g: Int, b: Int)) -> [Double] {
        guard bin.count > 0 else { return [0, 0, 0] }
        return [Double(bin.r), Double(bin.g), Double(bin.b)].map { $0 / Double(bin.count) }
    }
    let dark = count == 0 || brightness / count < 145_000
    func hue(_ rgb: [Double]) -> [Double] {
        let low = rgb.min()!, high = rgb.max()!
        guard high - low >= 18 else { return [0, 0, 0] }
        return rgb.map { ($0 - low) / (high - low) }
    }
    let baseHue = hue(average(dominant))
    // Let a bright, contrasting hue in a smaller part of the image color the outgoing bubble.
    func accentScore(_ bin: (count: Int, r: Int, g: Int, b: Int)) -> Double {
        let rgb = average(bin)
        let chroma = rgb.max()! - rgb.min()!
        let colorHue = hue(rgb)
        let distance = zip(colorHue, baseHue).reduce(0.0) { $0 + abs($1.0 - $1.1) }
        return chroma * log1p(Double(bin.count)) * (1.0 + distance)
    }
    let salient = bins.filter { $0.count >= max(1, count / 300) }.max { accentScore($0) < accentScore($1) }
    let accentHue = hue(salient.map(average) ?? [0, 0, 0])
    func color(_ hue: [Double], _ base: Double, _ amount: Double) -> String {
        let v = hue.map { min(255, max(0, Int((base + amount * $0).rounded()))) }
        return String(format: "#%02x%02x%02x", v[0], v[1], v[2])
    }
    let bg = color(baseHue, dark ? 14 : 239, dark ? 13 : 12)
    let incoming = color(baseHue, dark ? 26 : 248, dark ? 19 : 7)
    let outgoing = color(accentHue, dark ? 27 : 215, dark ? 36 : 30)
    let selected = color(accentHue, dark ? 48 : 200, dark ? 36 : 30)
    let accent = color(accentHue, dark ? 190 : 24, dark ? 60 : 38)
    let text = dark ? "#f8faf9" : "#172322"
    let muted = dark ? "#becbc9" : "#485b59"
    let safeName = name.components(separatedBy: .newlines).joined(separator: " ").replacingOccurrences(of: "=", with: " ").trimmingCharacters(in: .whitespaces)
    var lines = ["isDark = \(dark ? 1 : 0)", "tinted = 1", "name = \(safeName.isEmpty ? "Video Theme" : safeName)", "parent = \(dark ? "nightAccent" : "dayClassic")"]
    let groups: [([String], String)] = [
        (["background", "chatBackground", "listBackground", "grayBackground"], bg),
        (["bubbleBackground_incoming"], incoming), (["bubbleBackgroundTop_outgoing", "bubbleBackground_outgoing"], outgoing),
        (["accentSelect", "selectText", "bubbleBackgroundHighlight_incoming", "bubbleBackgroundHighlight_outgoing", "selectTextBubble_incoming", "selectTextBubble_outgoing", "grayHighlight", "chatDateActive", "badge", "badgeMuted"], selected),
        (["basicAccent", "accent", "link", "accentIcon", "linkBubble_incoming", "linkBubble_outgoing", "monospacedPre", "monospacedCode", "monospacedPreBubble_incoming", "monospacedCodeBubble_incoming", "monospacedPreBubble_outgoing", "monospacedCodeBubble_outgoing", "chatReplyTitle", "chatReplyTitleBubble_incoming", "chatReplyTitleBubble_outgoing"], accent),
        (["text", "textBubble_incoming", "textBubble_outgoing", "chatDateText", "indicatorColor", "chatReplyTextEnabled", "chatReplyTextEnabledBubble_incoming", "chatReplyTextEnabledBubble_outgoing"], text),
        (["grayText", "darkGrayText", "listGrayText", "grayTextBubble_incoming", "grayTextBubble_outgoing", "chatReplyTextDisabled", "chatReplyTextDisabledBubble_incoming", "chatReplyTextDisabledBubble_outgoing", "grayIcon", "grayIconBubble_incoming", "grayIconBubble_outgoing"], muted)
    ]
    for (keys, value) in groups { for key in keys { lines.append("\(key) = \(value)") } }
    lines.append("wallpaper = none")
    return lines.joined(separator: "\n") + "\n"
}
