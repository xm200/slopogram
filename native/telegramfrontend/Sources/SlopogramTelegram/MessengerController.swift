import AppKit
import Combine
import ColorPalette
import CoreImage.CIFilterBuiltins
import SlopogramState
import TGUIKit

final class AppearanceRootView: NSView {
    var appearanceChanged: (() -> Void)?
    override func viewDidChangeEffectiveAppearance() { super.viewDidChangeEffectiveAppearance(); appearanceChanged?() }
}

@MainActor final class MessengerController: NSViewController, NSSearchFieldDelegate, NSTextViewDelegate {
    private let store: Store
    private var subscription: AnyCancellable?
    private let sidebarMaterial = NSVisualEffectView()
    private let sidebar = TelegramRowsView()
    private let history = TelegramRowsView()
    private let search = NSSearchField()
    private let header = NSTextField(labelWithString: "Select a conversation")
    private let connection = NSButton(title: "Reconnect", target: nil, action: nil)
    private let composer = NSTextView()
    private let composerScroll = NSScrollView()
    private let send = NSButton(title: "Send", target: nil, action: nil)
    private let status = NSTextField(labelWithString: "")
    private let restore = NSButton(title: "Restore unconfirmed message", target: nil, action: nil)
    private let login = NSView()
    private let loginTitle = NSTextField(labelWithString: "Sign in to Telegram")
    private let loginDetail = NSTextField(wrappingLabelWithString: "")
    private let loginField = NSTextField()
    private let passwordField = NSSecureTextField()
    private let qr = NSImageView()
    private let primary = NSButton(title: "Continue", target: nil, action: nil)
    private let secondary = NSButton(title: "", target: nil, action: nil)
    private var screenKey = ""
    private var loginValue = ""
    private var previousSelected: Int64?

    var fixtureIsRendered: Bool { view.window != nil && sidebar.rowCount == 2 && history.rowCount == 2 && view.bounds.width > 0 }
    init(store: Store) { self.store = store; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func loadView() {
        let root = AppearanceRootView()
        root.appearanceChanged = { [weak self] in self?.applyAppearance() }
        root.wantsLayer = true
        view = root
    }
    private func applyAppearance() {
        let dark = view.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let old = presentation
        updateTheme(PresentationTheme(colors: dark ? darkPalette : whitePalette, search: old.search, inputTheme: old.inputTheme))
        view.layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
    }
    override func viewDidLoad() {
        super.viewDidLoad()
        applyAppearance()
        sidebarMaterial.material = .sidebar
        sidebarMaterial.blendingMode = .withinWindow
        sidebarMaterial.state = .active
        view.addSubview(sidebarMaterial)
        view.addSubview(sidebar); view.addSubview(history); view.addSubview(search); view.addSubview(header)
        view.addSubview(connection); view.addSubview(composerScroll); view.addSubview(send); view.addSubview(status); view.addSubview(restore)
        view.addSubview(login)
        [loginTitle, loginDetail, loginField, passwordField, qr, primary, secondary].forEach(login.addSubview)
        header.font = .systemFont(ofSize: 15, weight: .semibold)
        loginTitle.font = .systemFont(ofSize: 22, weight: .semibold)
        loginDetail.textColor = .secondaryLabelColor
        search.placeholderString = "Search conversations"
        search.delegate = self
        sidebar.onSelect = { [weak self] id in if let id = id.base as? Int64 { self?.store.select(id) } }
        composer.isRichText = false; composer.font = .systemFont(ofSize: 14); composer.delegate = self
        composerScroll.documentView = composer; composerScroll.hasVerticalScroller = true; composerScroll.borderType = .bezelBorder
        send.target = self; send.action = #selector(sendMessage)
        primary.target = self; primary.action = #selector(loginAction)
        secondary.target = self; secondary.action = #selector(alternateLogin)
        connection.target = self; connection.action = #selector(reconnect)
        restore.target = self; restore.action = #selector(restoreMessage)
        loginField.delegate = self; passwordField.delegate = self
        loginField.target = self; loginField.action = #selector(loginAction)
        passwordField.target = self; passwordField.action = #selector(loginAction)
        subscription = store.objectWillChange.sink { [weak self] _ in DispatchQueue.main.async { self?.render() } }
        NotificationCenter.default.addObserver(self, selector: #selector(becameActive), name: NSWindow.didBecomeKeyNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(resignedActive), name: NSWindow.didResignKeyNotification, object: nil)
        NSWorkspace.shared.notificationCenter.addObserver(self, selector: #selector(updateTransparency), name: NSWorkspace.accessibilityDisplayOptionsDidChangeNotification, object: nil)
        updateTransparency()
        render()
    }
    @objc private func becameActive(_ notification: Notification) { guard notification.object as? NSWindow === view.window else { return }; store.focused = true; store.markRead() }
    @objc private func resignedActive(_ notification: Notification) { guard notification.object as? NSWindow === view.window else { return }; store.focused = false }
    @objc private func updateTransparency() { sidebarMaterial.isHidden = NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency }
    override func viewDidAppear() { super.viewDidAppear(); render() }
    override func viewDidLayout() {
        super.viewDidLayout()
        let w = view.bounds.width, h = view.bounds.height, side: CGFloat = 300
        sidebarMaterial.frame = NSRect(x: 0, y: 0, width: side, height: h)
        search.frame = NSRect(x: 12, y: h - 58, width: side - 24, height: 28)
        sidebar.frame = NSRect(x: 0, y: 0, width: side, height: h - 66)
        header.frame = NSRect(x: side + 20, y: h - 53, width: w - side - 150, height: 22)
        connection.frame = NSRect(x: w - 125, y: h - 55, width: 110, height: 26)
        history.frame = NSRect(x: side, y: 100, width: w - side, height: max(0, h - 168))
        status.frame = NSRect(x: side + 18, y: 78, width: w - side - 230, height: 18)
        restore.frame = NSRect(x: w - 214, y: 74, width: 200, height: 24)
        composerScroll.frame = NSRect(x: side + 16, y: 16, width: w - side - 100, height: 50)
        send.frame = NSRect(x: w - 75, y: 23, width: 60, height: 30)
        login.frame = view.bounds
        let x = (w - 320) / 2
        let y = (h - (screenKey == "qr" ? 400 : 340)) / 2
        loginTitle.frame = NSRect(x: x, y: y + (screenKey == "qr" ? 360 : 290), width: 320, height: 30)
        loginDetail.frame = NSRect(x: x, y: y + (screenKey == "qr" ? 315 : 247), width: 320, height: 40)
        loginField.frame = NSRect(x: x, y: y + 207, width: 320, height: 28)
        passwordField.frame = loginField.frame
        qr.frame = NSRect(x: x + 39, y: y + 58, width: 242, height: 242)
        primary.frame = NSRect(x: x, y: y + 160, width: 320, height: 30)
        secondary.frame = NSRect(x: x, y: y + (screenKey == "qr" ? 8 : 120), width: 320, height: 30)
    }
    func controlTextDidChange(_ notification: Notification) { if notification.object as? NSSearchField === search { renderRows() } }
    func textDidChange(_ notification: Notification) { if let id = store.selected { store.drafts[id] = composer.string } }
    func textView(_ textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard commandSelector == #selector(NSResponder.insertNewline(_:)), !textView.hasMarkedText(), !NSEvent.modifierFlags.contains(.shift) else { return false }
        sendMessage(); return true
    }
    @objc private func sendMessage() { guard store.connected, !store.isReconnecting, store.hasActiveEngine, !composer.hasMarkedText() else { return }; store.send(); composer.string = store.selected.flatMap { store.drafts[$0] } ?? "" }
    @objc private func reconnect() { store.reconnect() }
    @objc private func restoreMessage() { if let id = store.selected, let key = store.failed.keys.sorted().first(where: { store.failed[$0]?.0 == id }) { store.recover(key); composer.string = store.drafts[id] ?? "" } }
    @objc private func loginAction() {
        switch store.screen {
        case .phone: let text = loginField.stringValue; store.command { try await $0.requestCode(phone: text) }
        case .code: let text = loginField.stringValue; loginField.stringValue = ""; store.command { try await $0.submitCode(code: text) }
        case .password: let text = passwordField.stringValue; passwordField.stringValue = ""; store.command { try await $0.submitPassword(password: text) }
        default: break
        }
    }
    @objc private func alternateLogin() {
        if case .phone = store.screen { store.command { try await $0.requestQrLogin() } }
        else { store.command { try await $0.resetLogin() } }
    }
    private func render() {
        let key: String
        switch store.screen {
        case .starting: key = "starting"
        case .phone: key = "phone"
        case .code: key = "code"
        case .password: key = "password"
        case .qr: key = "qr"
        case .chats: key = "chats"
        }
        let showingChats = key == "chats"
        login.isHidden = showingChats
        sidebarMaterial.isHidden = !showingChats || NSWorkspace.shared.accessibilityDisplayShouldReduceTransparency
        [sidebar, history, search, header, composerScroll, send, status, restore, connection].forEach { $0.isHidden = !showingChats }
        if key != screenKey {
            screenKey = key
            view.needsLayout = true
            loginField.stringValue = ""; passwordField.stringValue = ""
            loginTitle.stringValue = key == "starting" ? "Slopogram" : "Sign in to Telegram"
            loginField.isHidden = !["phone", "code"].contains(key)
            passwordField.isHidden = key != "password"
            qr.isHidden = key != "qr"
            primary.isHidden = !["phone", "code", "password"].contains(key)
            secondary.isHidden = !["phone", "code", "password", "qr"].contains(key)
            secondary.title = key == "phone" ? "Sign in with QR code" : "Use phone number"
            loginField.placeholderString = key == "phone" ? "Phone number with country code" : "Login code"
            passwordField.placeholderString = "Password"
        }
        switch store.screen {
        case .starting: loginDetail.stringValue = store.setupMessage ?? (store.connected ? "Connecting…" : "Connection interrupted. Restart Slopogram to reconnect.")
        case .phone: loginDetail.stringValue = "Enter your phone number with its country code."
        case .code: loginDetail.stringValue = "Enter the code sent by Telegram."
        case .password(let hint): loginDetail.stringValue = hint.isEmpty ? "Enter your two-step verification password." : hint
        case .qr(let url):
            loginDetail.stringValue = "Scan this code from Telegram on your phone."
            let filter = CIFilter.qrCodeGenerator(); filter.message = Data(url.utf8)
            if let output = filter.outputImage, let image = CIContext().createCGImage(output, from: output.extent) {
                let padded = NSImage(size: NSSize(width: 242, height: 242))
                padded.lockFocus()
                NSColor.white.setFill(); NSRect(x: 0, y: 0, width: 242, height: 242).fill()
                NSGraphicsContext.current?.imageInterpolation = .none
                let modules = CGFloat(image.width)
                let side = modules * max(1, floor(210 / modules))
                NSImage(cgImage: image, size: NSSize(width: modules, height: modules)).draw(in: NSRect(x: (242 - side) / 2, y: (242 - side) / 2, width: side, height: side))
                padded.unlockFocus()
                qr.image = padded; qr.imageScaling = .scaleProportionallyUpOrDown
                qr.wantsLayer = true; qr.layer?.magnificationFilter = .nearest
                qr.setAccessibilityLabel("Telegram login QR code")
            }
        case .chats: break
        }
        if showingChats {
            if previousSelected != store.selected { history.update([]) }
            renderRows()
            connection.isHidden = store.connected
            connection.isEnabled = !store.isReconnecting
            if previousSelected != store.selected {
                previousSelected = store.selected
                DispatchQueue.main.async { [weak self] in self?.history.scrollToEnd() }
                composer.string = store.selected.flatMap { store.drafts[$0] } ?? ""
            }
            let active = store.selected != nil
            composerScroll.isHidden = !active; send.isHidden = !active
            send.isEnabled = active && store.connected && !store.isReconnecting && store.hasActiveEngine
            let failed = store.failed.values.contains { $0.0 == store.selected }
            restore.isHidden = !failed
            let pending = store.pending.values.contains { $0.0 == store.selected }
            status.stringValue = pending ? "Sending…" : (failed ? "Message unconfirmed" : "")
        }
        if let message = store.error, let window = view.window, window.attachedSheet == nil {
            store.error = nil
            let alert = NSAlert(); alert.messageText = "Slopogram"; alert.informativeText = message; alert.addButton(withTitle: "OK")
            alert.beginSheetModal(for: window) { [weak self] _ in self?.render() }
        }
    }
    private func renderRows() {
        let query = search.stringValue
        sidebar.update(store.chats.filter { query.isEmpty || $0.title.localizedCaseInsensitiveContains(query) }.map {
            TelegramRowData(id: AnyHashable($0.id), title: $0.title, text: $0.preview, outgoing: false, conversation: true, selected: $0.id == store.selected)
        })
        if let id = store.selected {
            header.stringValue = store.chats.first(where: { $0.id == id })?.title ?? "Conversation"
            history.update((store.timelines[id] ?? [:]).values.sorted { $0.id < $1.id }.map {
                TelegramRowData(id: AnyHashable("\(id):\($0.id)"), title: $0.outgoing ? "You" : $0.sender, text: $0.text, outgoing: $0.outgoing, conversation: false, selected: false)
            }, followLatest: true)
        } else { header.stringValue = "Select a conversation"; history.update([]) }
    }
}
