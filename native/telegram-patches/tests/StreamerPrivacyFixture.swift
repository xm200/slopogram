import AppKit
import Foundation

final class FixtureDelegate: NSObject, NSApplicationDelegate {
    @objc func slopogramImportTheme(_ sender: Any?) {}
}

@main struct StreamerPrivacyFixture {
    static func main() {
        let profile = "fixture-" + UUID().uuidString
        let key = "Slopogram.Streamer." + profile
        defer { UserDefaults.standard.removeObject(forKey: key) }
        let startup = CommandLine.arguments.contains("--saved-active")
        if startup { UserDefaults.standard.set(true, forKey: key) }
        SlopogramStreamerMode.install(profileKey: profile)
        let app = NSApplication.shared
        let delegate = FixtureDelegate()
        app.delegate = delegate
        let menu = NSMenu()
        let root = NSMenuItem()
        root.submenu = NSMenu(title: "Synthetic")
        menu.addItem(root)
        app.mainMenu = menu
        if !startup {
            let action = NSSelectorFromString("slopogramImportTheme:")
            precondition(app.mainMenu?.item(at: 0)?.submenu?.items.contains(where: { $0.action == action }) == true)
            precondition(app.target(forAction: action, to: nil, from: nil) as AnyObject? === delegate)
        }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 240), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        window.title = "Synthetic private title"
        let privateView = NSTextField(labelWithString: "Synthetic private message")
        window.contentView?.addSubview(privateView)
        window.orderFront(nil)
        app.dockTile.badgeLabel = "123"
        let dockContent = NSView(frame: NSRect(x: 0, y: 0, width: 32, height: 32))
        app.dockTile.contentView = dockContent
        if startup { SlopogramStreamerMode.enforceStartup(app) }
        if !startup { SlopogramStreamerMode.setForTesting(true) }
        precondition(window.title != "Synthetic private title")
        precondition(privateView.isHidden)
        precondition(app.mainMenu !== menu)
        precondition(app.dockTile.badgeLabel == nil && app.dockTile.contentView == nil)
        SlopogramStreamerMode.setForTesting(true)
        let added = NSTextField(labelWithString: "Synthetic new content")
        window.contentView?.addSubview(added)
        precondition(added.isHidden)
        let safeMenu = app.mainMenu!
        let safeCount = safeMenu.numberOfItems
        safeMenu.addItem(NSMenuItem(title: "Synthetic private account", action: nil, keyEquivalent: ""))
        precondition(safeMenu.numberOfItems == safeCount)
        window.title = "Updated private title"
        window.subtitle = "Updated private subtitle"
        precondition(window.title == "Slopogram" && window.subtitle.isEmpty)
        let toolbar = NSToolbar(identifier: "fixture-toolbar")
        window.toolbar = toolbar
        toolbar.isVisible = true
        precondition(!toolbar.isVisible)
        let second = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 160), styleMask: [.titled], backing: .buffered, defer: false)
        second.title = "Another private title"
        let secondText = NSTextField(labelWithString: "Another private message")
        second.contentView?.addSubview(secondText)
        second.orderFront(nil)
        precondition(second.title == "Slopogram" && secondText.isHidden)
        let replacement = NSView(frame: NSRect(x: 0, y: 0, width: 320, height: 240))
        let replacementText = NSTextField(labelWithString: "Replacement private message")
        replacement.addSubview(replacementText)
        window.contentView = replacement
        precondition(replacementText.isHidden)
        SlopogramStreamerMode.setForTesting(false)
        SlopogramStreamerMode.setForTesting(false)
        precondition(window.title == "Updated private title" && window.subtitle == "Updated private subtitle")
        precondition(!replacementText.isHidden && !secondText.isHidden)
        precondition(toolbar.isVisible)
        second.close()
        precondition(window.title == "Updated private title")
        precondition(!privateView.isHidden && !added.isHidden)
        precondition(app.mainMenu === menu)
        let action = NSSelectorFromString("slopogramImportTheme:")
        precondition(app.mainMenu?.item(at: 0)?.submenu?.items.contains(where: { $0.action == action }) == true)
        precondition(app.target(forAction: action, to: nil, from: nil) as AnyObject? === delegate)
        precondition(app.dockTile.badgeLabel == "123" && app.dockTile.contentView === dockContent)
        window.close()
        print("Streamer synthetic window checks passed")
    }
}
