import SwiftUI
import RustBridge
import AppKit
import CoreImage.CIFilterBuiltins

struct ConversationRow: View {
    let title: String
    let preview: String
    var body: some View {
        HStack(spacing: 11) {
            Text(String(title.prefix(1)).uppercased())
                .font(.headline).foregroundStyle(.white)
                .frame(width: 42, height: 42)
                .background(.blue.gradient, in: Circle())
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.headline).lineLimit(1)
                Text(preview).font(.subheadline).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 0)
        }.padding(.vertical, 5)
    }
}

struct QRImage: View {
    let value: String
    var body: some View {
        Group {
            if let image = makeImage() {
                Image(nsImage: image).interpolation(.none).resizable()
                    .frame(width: 210, height: 210)
                    .padding(16).background(.white, in: RoundedRectangle(cornerRadius: 12))
            } else {
                ProgressView().frame(width: 242, height: 242)
            }
        }
        .accessibilityLabel("Telegram login QR code")
    }
    private func makeImage() -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(value.utf8)
        guard let output = filter.outputImage,
              let cg = CIContext().createCGImage(output, from: output.extent) else { return nil }
        return NSImage(cgImage: cg, size: NSSize(width: 210, height: 210))
    }
}

struct MessageBubble: View {
    let text: String
    let sender: String
    let outgoing: Bool
    var body: some View {
        HStack(alignment: .bottom) {
            if outgoing { Spacer(minLength: 60) }
            VStack(alignment: .leading, spacing: 4) {
                if !outgoing && !sender.isEmpty { Text(sender).font(.caption.weight(.semibold)).foregroundStyle(.blue) }
                Text(text).textSelection(.enabled)
            }
            .padding(.horizontal, 12).padding(.vertical, 8)
            .background(outgoing ? Color.accentColor.opacity(0.18) : Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 14))
            if !outgoing { Spacer(minLength: 60) }
        }.padding(.horizontal, 20).padding(.vertical, 2)
    }
}

/// A native multiline editor: Return sends, Shift-Return inserts a line break.
struct Composer: NSViewRepresentable {
    @Binding var text: String
    var send: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSTextView.scrollableTextView()
        scroll.drawsBackground = false
        scroll.hasVerticalScroller = true
        guard let view = scroll.documentView as? NSTextView else { return scroll }
        view.delegate = context.coordinator
        view.font = .systemFont(ofSize: 14)
        view.drawsBackground = false
        view.textContainerInset = NSSize(width: 8, height: 8)
        view.isRichText = false
        view.isAutomaticQuoteSubstitutionEnabled = false
        view.isAutomaticDashSubstitutionEnabled = false
        view.setContentHuggingPriority(.defaultLow, for: .horizontal)
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        context.coordinator.parent = self
        guard let view = scroll.documentView as? NSTextView, view.string != text else { return }
        view.string = text
    }
    final class Coordinator: NSObject, NSTextViewDelegate {
        var parent: Composer
        init(_ parent: Composer) { self.parent = parent }
        func textDidChange(_ notification: Notification) {
            guard let view = notification.object as? NSTextView else { return }
            parent.text = view.string
        }
        func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            guard commandSelector == #selector(NSResponder.insertNewline(_:)),
                  !textView.hasMarkedText(),
                  !NSEvent.modifierFlags.contains(.shift) else { return false }
            parent.send()
            return true
        }
    }
}

/// Reuses native table rows and keeps the visible scroll position during updates.
struct MessageList: NSViewRepresentable {
    let messages: [MessageRecord]
    func makeCoordinator() -> Coordinator { Coordinator() }
    func makeNSView(context: Context) -> NSScrollView {
        let scroll = NSScrollView()
        let table = NSTableView()
        table.headerView = nil
        table.backgroundColor = .clear
        table.selectionHighlightStyle = .none
        table.usesAutomaticRowHeights = true
        table.intercellSpacing = .zero
        table.addTableColumn(NSTableColumn(identifier: NSUserInterfaceItemIdentifier("message")))
        table.delegate = context.coordinator
        table.dataSource = context.coordinator
        scroll.documentView = table
        scroll.hasVerticalScroller = true
        scroll.drawsBackground = false
        return scroll
    }
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        guard let table = scroll.documentView as? NSTableView else { return }
        guard context.coordinator.messages != messages else { return }
        let origin = scroll.contentView.bounds.origin
        let nearBottom = table.bounds.height - scroll.contentView.bounds.maxY < 40
        let old = context.coordinator.messages
        let topRow = table.row(at: NSPoint(x: 0, y: origin.y))
        let anchor = old.indices.contains(topRow) ? old[topRow].id : nil
        let offset = topRow >= 0 ? origin.y - table.rect(ofRow: topRow).origin.y : 0
        context.coordinator.messages = messages
        let oldIDs = Set(old.map(\.id))
        let newIDs = Set(messages.map(\.id))
        let removed = IndexSet(old.indices.filter { !newIDs.contains(old[$0].id) })
        let inserted = IndexSet(messages.indices.filter { !oldIDs.contains(messages[$0].id) })
        let previous = Dictionary(uniqueKeysWithValues: old.map { ($0.id, $0) })
        let changed = IndexSet(messages.indices.filter { index in previous[messages[index].id].map { $0 != messages[index] } ?? false })
        table.beginUpdates()
        table.removeRows(at: removed, withAnimation: [])
        table.insertRows(at: inserted, withAnimation: [])
        table.endUpdates()
        if !changed.isEmpty { table.reloadData(forRowIndexes: changed, columnIndexes: IndexSet(integer: 0)) }
        table.layoutSubtreeIfNeeded()
        if nearBottom, !messages.isEmpty { table.scrollRowToVisible(messages.count - 1) }
        else {
            let y = anchor.flatMap { id in messages.firstIndex(where: { $0.id == id }) }.map { table.rect(ofRow: $0).origin.y + offset } ?? origin.y
            scroll.contentView.scroll(to: NSPoint(x: origin.x, y: y))
            scroll.reflectScrolledClipView(scroll.contentView)
        }
    }
    final class Coordinator: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        var messages: [MessageRecord] = []
        func numberOfRows(in tableView: NSTableView) -> Int { messages.count }
        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            let message = messages[row]
            let identifier = NSUserInterfaceItemIdentifier("message")
            let host = (tableView.makeView(withIdentifier: identifier, owner: nil) as? NSHostingView<MessageBubble>) ?? NSHostingView(rootView: MessageBubble(text: "", sender: "", outgoing: false))
            host.identifier = identifier
            host.rootView = MessageBubble(text: message.text, sender: message.sender, outgoing: message.outgoing)
            return host
        }
    }
}

struct ComposerSurface: ViewModifier {
    @Environment(\.accessibilityReduceTransparency) private var reduceTransparency
    func body(content: Content) -> some View {
        if reduceTransparency {
            content.background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
        } else if #available(macOS 26, *) {
            content.glassEffect(.regular, in: RoundedRectangle(cornerRadius: 10))
        } else {
            content.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        }
    }
}
