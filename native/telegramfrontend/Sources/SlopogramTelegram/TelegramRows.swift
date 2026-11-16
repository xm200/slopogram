import AppKit
import TGUIKit

struct TelegramRowData: Equatable {
    let id: AnyHashable
    let title: String
    let text: String
    let outgoing: Bool
    let conversation: Bool
    let selected: Bool
}

final class TelegramRowItem: TableRowItem {
    let data: TelegramRowData
    let action: (() -> Void)?

    init(_ data: TelegramRowData, size: NSSize, action: (() -> Void)? = nil) {
        self.data = data
        self.action = action
        super.init(size, stableId: data.id)
    }

    override var height: CGFloat {
        guard !data.conversation else { return 70 }
        let available = max(120, min(width - 80, 520))
        let size = (data.text as NSString).boundingRect(with: NSSize(width: available - 24, height: .greatestFiniteMagnitude), options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: [.font: NSFont.systemFont(ofSize: 13)]).size
        return max(50, ceil(size.height) + 42)
    }
    override var backdorColor: NSColor { data.conversation && data.selected ? NSColor.controlAccentColor.withAlphaComponent(0.15) : .clear }
    override func viewClass() -> AnyClass { TelegramRowView.self }
}

final class TelegramRowView: TableRowView {
    private let bubble = NSView()
    private let title = NSTextField(labelWithString: "")
    private let text = NSTextField(wrappingLabelWithString: "")
    private let avatar = NSTextField(labelWithString: "")

    required init(frame: NSRect) {
        super.init(frame: frame)
        bubble.wantsLayer = true
        bubble.layer?.cornerRadius = 12
        addSubview(bubble)
        title.font = .systemFont(ofSize: 13, weight: .semibold)
        text.font = .systemFont(ofSize: 13)
        text.textColor = .secondaryLabelColor
        text.maximumNumberOfLines = 0
        text.isSelectable = true
        avatar.font = .systemFont(ofSize: 17, weight: .medium)
        avatar.textColor = .white
        avatar.alignment = .center
        avatar.wantsLayer = true
        avatar.layer?.backgroundColor = NSColor.systemBlue.cgColor
        avatar.layer?.cornerRadius = 22
        addSubview(avatar)
        addSubview(title)
        addSubview(text)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func set(item: TableRowItem, animated: Bool = false) {
        super.set(item: item, animated: animated)
        guard let item = item as? TelegramRowItem else { return }
        title.stringValue = item.data.title
        text.stringValue = item.data.text
        text.isSelectable = !item.data.conversation
        avatar.stringValue = String(item.data.title.prefix(1)).uppercased()
        avatar.isHidden = !item.data.conversation
        setAccessibilityLabel(item.data.conversation ? "\(item.data.title), \(item.data.text)" : "\(item.data.title): \(item.data.text)")
        setAccessibilitySelected(item.data.selected)
        bubble.isHidden = item.data.conversation
        updateBubbleColor(item)
        title.textColor = item.data.conversation ? .labelColor : .controlAccentColor
        needsLayout = true
    }
    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        if let item = item as? TelegramRowItem { updateBubbleColor(item) }
    }
    private func updateBubbleColor(_ item: TelegramRowItem) {
        effectiveAppearance.performAsCurrentDrawingAppearance {
            bubble.layer?.backgroundColor = (item.data.outgoing ? NSColor.controlAccentColor.withAlphaComponent(0.16) : NSColor.controlBackgroundColor).cgColor
        }
    }
    override func layout() {
        super.layout()
        guard let item = item as? TelegramRowItem else { return }
        avatar.frame = NSRect(x: 12, y: (bounds.height - 44) / 2, width: 44, height: 44)
        let width = item.data.conversation ? bounds.width - 82 : min(bounds.width - 80, 520)
        let x: CGFloat = item.data.conversation ? 66 : (item.data.outgoing ? bounds.width - width - 18 : 18)
        bubble.frame = NSRect(x: x - 8, y: 2, width: max(0, width + 16), height: bounds.height - 4)
        title.frame = NSRect(x: x, y: bounds.height - 25, width: max(0, width), height: 18)
        text.frame = NSRect(x: x, y: 7, width: max(0, width), height: max(18, bounds.height - 34))
    }
    override func mouseUp(with event: NSEvent) {
        if let item = item as? TelegramRowItem, let action = item.action { action() }
        else { super.mouseUp(with: event) }
    }
}

final class TelegramRowsView: NSView {
    private let table = TableView(frame: .zero)
    private var rows: [TelegramRowData] = []
    var onSelect: ((AnyHashable) -> Void)?
    var rowCount: Int { table.count }

    override init(frame: NSRect) {
        super.init(frame: frame)
        table.drawsBackground = false
        table.backgroundColor = .clear
        addSubview(table)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func layout() { super.layout(); table.frame = bounds }

    func update(_ newRows: [TelegramRowData], followLatest: Bool = false) {
        let visible = table.visibleItems()
        let follow = followLatest && shouldFollowLatest(oldCount: rows.count, newCount: newRows.count, wasAtBottom: table.isAtBottom)
        let delta = TelegramRowDelta(old: rows, new: newRows)
        for index in delta.removed.reversed() {
            table.remove(at: index, animation: .none)
            rows.remove(at: index)
        }
        for (index, row) in newRows.enumerated() {
            if index < rows.count, rows[index].id == row.id {
                if rows[index] != row {
                    table.replace(item: makeItem(row), at: index, animated: false)
                    rows[index] = row
                }
            } else {
                if let oldIndex = rows.firstIndex(where: { $0.id == row.id }) {
                    table.remove(at: oldIndex, animation: .none)
                    rows.remove(at: oldIndex)
                }
                _ = table.insert(item: makeItem(row), at: index, animation: .none)
                rows.insert(row, at: index)
            }
        }
        if follow { scrollToEnd() } else { table.saveScrollState(visible) }
    }
    func scrollToEnd() {
        guard let id = rows.last?.id else { return }
        table.scroll(to: .bottom(id: id, innerId: nil, animated: false, focus: .init(focus: false), inset: 0))
    }
    private func makeItem(_ row: TelegramRowData) -> TelegramRowItem {
        TelegramRowItem(row, size: bounds.size) { [weak self] in self?.onSelect?(row.id) }
    }
}

struct TelegramRowDelta {
    let removed: [Int]
    let inserted: [Int]
    let changed: [Int]
    init(old: [TelegramRowData], new: [TelegramRowData]) {
        let oldByID = Dictionary(uniqueKeysWithValues: old.map { ($0.id, $0) })
        let newIDs = Set(new.map(\.id))
        removed = old.indices.filter { !newIDs.contains(old[$0].id) }
        inserted = new.indices.filter { oldByID[new[$0].id] == nil }
        changed = new.indices.filter { index in oldByID[new[index].id].map { $0 != new[index] } ?? false }
    }
}

func shouldFollowLatest(oldCount: Int, newCount: Int, wasAtBottom: Bool) -> Bool {
    newCount > 0 && (oldCount == 0 || (newCount > oldCount && wasAtBottom))
}
