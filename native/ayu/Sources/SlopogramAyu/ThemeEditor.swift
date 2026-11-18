import AppKit
import ImageIO
import UniformTypeIdentifiers

func normalizedDesktopThemeHex(_ value: String) -> String? {
  let raw = value.trimmingCharacters(in: .whitespacesAndNewlines)
  let digits = raw.hasPrefix("#") ? String(raw.dropFirst()) : raw
  guard digits.count == 6 || digits.count == 8,
    digits.allSatisfy({ $0.isASCII && $0.isHexDigit })
  else { return nil }
  return "#" + digits.uppercased()
}

func prepareEditorWallpaper(url: URL) async throws -> (Data, String, Data?) {
  try await Task.detached { () async throws -> (Data, String, Data?) in
    let scoped = url.startAccessingSecurityScopedResource()
    defer { if scoped { url.stopAccessingSecurityScopedResource() } }
    if url.pathExtension.lowercased() == "mp4" {
      let source = try await prepareVideoThemeSource(url: url)
      return (source.poster, "jpg", source.video)
    }
    let size =
      (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?
      .intValue ?? 0
    guard size > 0 && size <= 16 * 1024 * 1024 else { throw CocoaError(.fileReadCorruptFile) }
    let data = try Data(contentsOf: url)
    guard let source = CGImageSourceCreateWithData(data as CFData, nil),
      let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
      let width = properties[kCGImagePropertyPixelWidth] as? Int,
      let height = properties[kCGImagePropertyPixelHeight] as? Int, width > 0, height > 0,
      width <= 8192, height <= 8192, width * height <= 40_000_000
    else { throw CocoaError(.fileReadCorruptFile) }
    return (
      data, url.pathExtension.lowercased() == "jpeg" ? "jpg" : url.pathExtension.lowercased(),
      nil
    )
  }.value
}

private final class FlippedDocumentView: NSView {
  override var isFlipped: Bool { true }
}

/// Edits a desktop palette without changing the active theme until Apply is chosen.
@MainActor
public final class SlopogramThemeEditorController: NSWindowController, NSWindowDelegate,
  NSTextFieldDelegate
{
  private var colors: [DesktopThemeColor] = []
  private var wallpaper: Data?
  private var wallpaperExtension: String?
  private var tiled: Bool
  private var video: Data?
  private let applyHandler: (Data, String) -> Void
  public var onClose: (() -> Void)?
  private let search = NSSearchField()
  private let name = NSTextField(string: "My Theme")
  private let rows = NSStackView()
  private let preview = NSStackView()
  private let status = NSTextField(labelWithString: "")
  private let applyButton = NSButton(title: "Apply", target: nil, action: nil)
  private let exportButton = NSButton(title: "Export…", target: nil, action: nil)
  private let chooseButton = NSButton(title: "Choose Wallpaper…", target: nil, action: nil)
  private var fields: [Int: NSTextField] = [:]
  private var wells: [Int: NSColorWell] = [:]
  private var drafts: [Int: String] = [:]
  private var invalidFields: Set<Int> = []
  private var closed = false
  private var loaded = false
  private var busy = false {
    didSet {
      applyButton.isEnabled = !busy && loaded
      exportButton.isEnabled = !busy && loaded
      chooseButton.isEnabled = !busy && loaded
      search.isEnabled = !busy
      name.isEnabled = !busy
      for field in fields.values { field.isEnabled = !busy }
      for row in rows.arrangedSubviews {
        for well in row.subviews.compactMap({ $0 as? NSColorWell }) { well.isEnabled = !busy }
      }
    }
  }

  public init(
    palette: String, wallpaper: Data?, wallpaperExtension: String?, wallpaperTiled: Bool,
    video: Data?, onApply: @escaping (Data, String) -> Void
  ) {
    self.wallpaper = wallpaper
    self.wallpaperExtension = wallpaperExtension
    self.tiled = wallpaperTiled
    self.video = video
    self.applyHandler = onApply
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 900, height: 650),
      styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
    super.init(window: window)
    window.title = "Theme Editor"
    window.minSize = NSSize(width: 700, height: 500)
    window.delegate = self
    buildUI()
    busy = true
    Task { [weak self] in
      do {
        let result = try await desktopThemeColors(palette: palette)
        guard let self, !self.closed else { return }
        self.colors = result
        self.loaded = true
        self.busy = false
        self.refresh()
      } catch {
        self?.showError(error)
        self?.busy = false
      }
    }
  }
  required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
  public func windowWillClose(_ notification: Notification) {
    closed = true
    onClose?()
  }

  private func buildUI() {
    guard let content = window?.contentView else { return }
    let root = NSStackView()
    root.orientation = .vertical
    root.spacing = 12
    root.translatesAutoresizingMaskIntoConstraints = false
    content.addSubview(root)
    NSLayoutConstraint.activate([
      root.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 20),
      root.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -20),
      root.topAnchor.constraint(equalTo: content.topAnchor, constant: 20),
      root.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),
    ])
    let header = NSStackView(views: [
      NSTextField(labelWithString: "Theme name"), name, chooseButton,
    ])
    header.spacing = 10
    chooseButton.target = self
    chooseButton.action = #selector(chooseWallpaper)
    root.addArrangedSubview(header)
    header.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true
    let body = NSStackView()
    body.orientation = .horizontal
    body.spacing = 20
    body.alignment = .top
    root.addArrangedSubview(body)
    body.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true
    body.heightAnchor.constraint(greaterThanOrEqualToConstant: 300).isActive = true
    let left = NSStackView()
    left.orientation = .vertical
    left.spacing = 8
    body.addArrangedSubview(left)
    left.widthAnchor.constraint(equalTo: body.widthAnchor, multiplier: 0.53).isActive = true
    search.placeholderString = "Search colors"
    search.target = self
    search.action = #selector(searchChanged)
    left.addArrangedSubview(search)
    search.widthAnchor.constraint(equalTo: left.widthAnchor).isActive = true
    let scroll = NSScrollView()
    scroll.hasVerticalScroller = true
    scroll.borderType = .bezelBorder
    left.addArrangedSubview(scroll)
    scroll.widthAnchor.constraint(equalTo: left.widthAnchor).isActive = true
    scroll.heightAnchor.constraint(equalTo: body.heightAnchor, constant: -32).isActive = true
    rows.orientation = .vertical
    rows.alignment = .leading
    rows.spacing = 4
    rows.translatesAutoresizingMaskIntoConstraints = false
    let document = FlippedDocumentView()
    document.translatesAutoresizingMaskIntoConstraints = false
    document.addSubview(rows)
    scroll.documentView = document
    NSLayoutConstraint.activate([
      rows.leadingAnchor.constraint(equalTo: document.leadingAnchor, constant: 8),
      rows.trailingAnchor.constraint(equalTo: document.trailingAnchor, constant: -8),
      rows.topAnchor.constraint(equalTo: document.topAnchor, constant: 8),
      rows.bottomAnchor.constraint(equalTo: document.bottomAnchor, constant: -8),
      document.widthAnchor.constraint(equalTo: scroll.contentView.widthAnchor),
    ])
    preview.orientation = .vertical
    preview.spacing = 12
    preview.alignment = .leading
    body.addArrangedSubview(preview)
    let spacer = NSView()
    spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)
    let footer = NSStackView(views: [status, spacer, exportButton, applyButton])
    footer.spacing = 8
    root.addArrangedSubview(footer)
    footer.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true
    status.textColor = .secondaryLabelColor
    applyButton.bezelStyle = .rounded
    applyButton.keyEquivalent = "\r"
    applyButton.target = self
    applyButton.action = #selector(applyTheme)
    exportButton.target = self
    exportButton.action = #selector(exportTheme)
  }
  @objc private func searchChanged() { refreshRows() }
  private func refresh() {
    refreshRows()
    refreshPreview()
  }
  private func refreshRows() {
    rows.arrangedSubviews.forEach {
      rows.removeArrangedSubview($0)
      $0.removeFromSuperview()
    }
    fields.removeAll()
    wells.removeAll()
    let labels = [
      "windowBg": "Window background", "dialogsBg": "Chat list background",
      "msgInBg": "Incoming bubble", "msgOutBg": "Outgoing bubble",
      "historyTextInFg": "Incoming text", "historyTextOutFg": "Outgoing text",
      "windowFgActive": "Active text",
    ]
    for (index, color) in colors.enumerated()
    where search.stringValue.isEmpty
      || color.key.localizedCaseInsensitiveContains(search.stringValue)
      || (labels[color.key]?.localizedCaseInsensitiveContains(search.stringValue) ?? false)
    {
      let label = NSTextField(
        labelWithString: labels[color.key].map { "\($0) (\(color.key))" } ?? color.key)
      label.lineBreakMode = .byTruncatingMiddle
      let well = NSColorWell()
      well.color = parseColor(color.value) ?? .clear
      well.tag = index
      wells[index] = well
      well.target = self
      well.action = #selector(wellChanged(_:))
      let field = NSTextField(string: drafts[index] ?? color.value)
      field.tag = index
      field.delegate = self
      fields[index] = field
      field.target = self
      field.action = #selector(hexChanged(_:))
      field.widthAnchor.constraint(equalToConstant: 105).isActive = true
      let row = NSStackView(views: [label, well, field])
      row.spacing = 8
      rows.addArrangedSubview(row)
      row.widthAnchor.constraint(equalTo: rows.widthAnchor).isActive = true
    }
  }
  private func parseColor(_ value: String) -> NSColor? {
    let hex = value.trimmingCharacters(in: .whitespacesAndNewlines).trimmingCharacters(
      in: CharacterSet(charactersIn: "#"))
    guard hex.count == 6 || hex.count == 8, let n = UInt64(hex, radix: 16) else { return nil }
    let shift = hex.count == 8 ? 8 : 0
    return NSColor(
      srgbRed: CGFloat((n >> (16 + shift)) & 255) / 255,
      green: CGFloat((n >> (8 + shift)) & 255) / 255, blue: CGFloat((n >> shift) & 255) / 255,
      alpha: hex.count == 8 ? CGFloat(n & 255) / 255 : 1)
  }
  @objc private func wellChanged(_ sender: NSColorWell) {
    guard let c = sender.color.usingColorSpace(.sRGB), colors.indices.contains(sender.tag) else {
      return
    }
    colors[sender.tag] = DesktopThemeColor(
      key: colors[sender.tag].key,
      value: String(
        format: "#%02X%02X%02X%02X", Int(c.redComponent * 255), Int(c.greenComponent * 255),
        Int(c.blueComponent * 255), Int(c.alphaComponent * 255)))
    fields[sender.tag]?.stringValue = colors[sender.tag].value
    invalidFields.remove(sender.tag)
    drafts.removeValue(forKey: sender.tag)
    refreshPreview()
  }
  public func controlTextDidEndEditing(_ notification: Notification) {
    if let field = notification.object as? NSTextField, field !== search, field !== name {
      hexChanged(field)
    }
  }
  @objc private func hexChanged(_ sender: NSTextField) {
    guard colors.indices.contains(sender.tag) else { return }
    guard let normalized = normalizedDesktopThemeHex(sender.stringValue) else {
      invalidFields.insert(sender.tag)
      drafts[sender.tag] = sender.stringValue
      status.stringValue = "Enter a 6- or 8-digit hex color."
      return
    }
    sender.stringValue = normalized
    colors[sender.tag] = DesktopThemeColor(key: colors[sender.tag].key, value: normalized)
    invalidFields.remove(sender.tag)
    drafts.removeValue(forKey: sender.tag)
    if let color = parseColor(normalized) { wells[sender.tag]?.color = color }
    status.stringValue = ""
    refreshPreview()
  }
  private func commitEdits() -> Bool {
    window?.makeFirstResponder(nil)
    if !invalidFields.isEmpty {
      status.stringValue = "Correct the invalid hex color before continuing."
      return false
    }
    return true
  }
  private func refreshPreview() {
    preview.arrangedSubviews.forEach {
      preview.removeArrangedSubview($0)
      $0.removeFromSuperview()
    }
    func color(_ key: String, fallback: NSColor) -> NSColor {
      colors.first(where: { $0.key == key }).flatMap { parseColor($0.value) } ?? fallback
    }
    let title = NSTextField(labelWithString: "Preview")
    title.font = .boldSystemFont(ofSize: 16)
    preview.addArrangedSubview(title)
    let canvas = NSView()
    canvas.wantsLayer = true
    canvas.layer?.backgroundColor = color("windowBg", fallback: .windowBackgroundColor).cgColor
    canvas.layer?.cornerRadius = 8
    canvas.layer?.masksToBounds = true
    preview.addArrangedSubview(canvas)
    canvas.widthAnchor.constraint(equalToConstant: 290).isActive = true
    canvas.heightAnchor.constraint(equalToConstant: 300).isActive = true
    if let wallpaper, let image = NSImage(data: wallpaper) {
      let imageView = NSView()
      imageView.wantsLayer = true
      imageView.layer?.contents = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
      imageView.layer?.contentsGravity = .resizeAspectFill
      imageView.layer?.masksToBounds = true
      imageView.translatesAutoresizingMaskIntoConstraints = false
      canvas.addSubview(imageView)
      NSLayoutConstraint.activate([
        imageView.leadingAnchor.constraint(equalTo: canvas.leadingAnchor),
        imageView.trailingAnchor.constraint(equalTo: canvas.trailingAnchor),
        imageView.topAnchor.constraint(equalTo: canvas.topAnchor),
        imageView.bottomAnchor.constraint(equalTo: canvas.bottomAnchor),
      ])
    }
    let chat = NSStackView()
    chat.orientation = .vertical
    chat.alignment = .leading
    chat.spacing = 16
    chat.translatesAutoresizingMaskIntoConstraints = false
    canvas.addSubview(chat)
    NSLayoutConstraint.activate([
      chat.leadingAnchor.constraint(equalTo: canvas.leadingAnchor, constant: 16),
      chat.trailingAnchor.constraint(equalTo: canvas.trailingAnchor, constant: -16),
      chat.topAnchor.constraint(equalTo: canvas.topAnchor, constant: 16),
    ])
    let heading = NSTextField(labelWithString: "Sample conversation")
    heading.font = .boldSystemFont(ofSize: 14)
    heading.textColor = color("windowFgActive", fallback: .labelColor)
    heading.wantsLayer = true
    heading.layer?.backgroundColor = color("dialogsBg", fallback: .controlBackgroundColor).cgColor
    chat.addArrangedSubview(heading)
    for (text, background, foreground, outgoing) in [
      ("Hello! This is a preview.", "msgInBg", "historyTextInFg", false),
      ("Colors update as you edit.", "msgOutBg", "historyTextOutFg", true),
    ] {
      let bubble = NSView()
      bubble.wantsLayer = true
      bubble.layer?.backgroundColor = color(background, fallback: .controlBackgroundColor).cgColor
      bubble.layer?.cornerRadius = 10
      let label = NSTextField(wrappingLabelWithString: text)
      label.textColor = color(foreground, fallback: .labelColor)
      label.translatesAutoresizingMaskIntoConstraints = false
      bubble.addSubview(label)
      NSLayoutConstraint.activate([
        label.leadingAnchor.constraint(equalTo: bubble.leadingAnchor, constant: 10),
        label.trailingAnchor.constraint(equalTo: bubble.trailingAnchor, constant: -10),
        label.topAnchor.constraint(equalTo: bubble.topAnchor, constant: 8),
        label.bottomAnchor.constraint(equalTo: bubble.bottomAnchor, constant: -8),
      ])
      let line = NSStackView()
      line.orientation = .horizontal
      if outgoing { line.addArrangedSubview(NSView()) }
      line.addArrangedSubview(bubble)
      if !outgoing { line.addArrangedSubview(NSView()) }
      chat.addArrangedSubview(line)
      line.widthAnchor.constraint(equalTo: chat.widthAnchor).isActive = true
    }
    if video != nil { preview.addArrangedSubview(NSTextField(labelWithString: "Video wallpaper")) }
  }
  @objc private func chooseWallpaper() {
    let panel = NSOpenPanel()
    panel.allowedContentTypes = [.png, .jpeg, .mpeg4Movie]
    panel.allowsMultipleSelection = false
    guard panel.runModal() == .OK, let url = panel.url else { return }
    busy = true
    status.stringValue = "Preparing wallpaper…"
    Task { [weak self] in
      do {
        let result = try await prepareEditorWallpaper(url: url)
        guard let self, !self.closed else { return }
        self.wallpaper = result.0
        self.wallpaperExtension = result.1
        self.video = result.2
        self.tiled = false
        self.busy = false
        self.status.stringValue = ""
        self.refreshPreview()
      } catch {
        self?.busy = false
        self?.showError(error)
      }
    }
  }
  private func encoded() async throws -> Data {
    try await encodeDesktopTheme(
      colors: colors, wallpaper: wallpaper, wallpaperExtension: wallpaperExtension,
      wallpaperTiled: tiled, video: video)
  }
  @objc private func applyTheme() {
    guard commitEdits() else { return }
    let themeName = name.stringValue
    busy = true
    Task { [weak self] in
      guard let self else { return }
      do {
        let data = try await self.encoded()
        guard !self.closed else { return }
        self.applyHandler(data, themeName)
        self.busy = false
      } catch {
        self.busy = false
        self.showError(error)
      }
    }
  }
  @objc private func exportTheme() {
    guard commitEdits() else { return }
    let panel = NSSavePanel()
    panel.allowedContentTypes = [UTType(filenameExtension: "tdesktop-theme") ?? .data]
    panel.nameFieldStringValue = name.stringValue + ".tdesktop-theme"
    guard panel.runModal() == .OK, let url = panel.url else { return }
    busy = true
    Task { [weak self] in
      guard let self else { return }
      do {
        let data = try await self.encoded()
        guard !self.closed else { return }
        try await Task.detached {
          let scoped = url.startAccessingSecurityScopedResource()
          defer { if scoped { url.stopAccessingSecurityScopedResource() } }
          try data.write(to: url, options: .atomic)
        }.value
        self.busy = false
        self.status.stringValue = "Theme exported."
      } catch {
        self.busy = false
        self.showError(error)
      }
    }
  }
  private func showError(_ error: Error) {
    guard !closed else { return }
    status.stringValue = error.localizedDescription
    let alert = NSAlert(error: error)
    if let window { alert.beginSheetModal(for: window) }
  }
}
