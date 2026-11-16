//! Native macOS integration: standard menu bar, services menu, dock icon, and
//! system proxy detection, through AppKit and SystemConfiguration.
//!
//! Iced 0.14 exposes no menu-bar API, so this crate talks to AppKit directly.
//! Copy/cut/paste/select-all already work in Iced text inputs through keyboard
//! shortcuts, so the menu only carries the standard application and window
//! actions that route through NSApp's responder chain.
//!
//! Objective-C FFI is inherently `unsafe`; this crate deliberately does not
//! inherit the workspace `unsafe_code = "forbid"` lint. All unsafe calls are
//! confined to this file. On non-macOS targets this crate is empty, because
//! its AppKit dependency only builds on Apple platforms.
#![cfg(target_os = "macos")]

use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, rc::Retained, runtime::Sel, sel};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSColor, NSEventModifierFlags, NSImage, NSMenu,
    NSMenuItem, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
    NSVisualEffectView, NSWindowOrderingMode,
};
use objc2_foundation::{NSData, NSString};

/// Install the menu bar and dock icon. Call once from the main thread before
/// the window opens. Failures leave the app running without native extras.
pub fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("Slopogram: not on the main thread; skipping macOS integration.");
        return;
    };
    install_dock_icon(&mtm);
    install_menus(&mtm);
}

/// Install a native, appearance-aware AppKit backdrop in each Iced window.
/// Call after Iced creates or focuses its window; repeated calls are harmless.
pub fn install_backdrop() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    for window in app.windows().iter() {
        let Some(content) = window.contentView() else {
            continue;
        };
        // SAFETY: content is retained by this live window and accessed on the
        // main thread; the returned parent is retained by the binding.
        let Some(parent) = (unsafe { content.superview() }) else {
            continue;
        };
        if parent
            .subviews()
            .iter()
            .any(|view| view.downcast_ref::<NSVisualEffectView>().is_some())
        {
            continue;
        }

        let effect =
            NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), content.frame());
        effect.setMaterial(NSVisualEffectMaterial::Sidebar);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::FollowsWindowActiveState);
        effect.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        parent.addSubview_positioned_relativeTo(
            effect.as_ref(),
            NSWindowOrderingMode::Below,
            Some(&content),
        );
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
    }
}

fn install_dock_icon(mtm: &MainThreadMarker) {
    let icon = {
        let bytes = include_bytes!("../../../assets/slopogram-icon.png");
        NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes))
    };
    if let Some(icon) = icon {
        // SAFETY: the icon outlives the call; AppKit retains it.
        unsafe {
            NSApplication::sharedApplication(*mtm).setApplicationIconImage(Some(&icon));
        }
    }
}

fn install_menus(mtm: &MainThreadMarker) {
    let app = NSApplication::sharedApplication(*mtm);
    let main_menu = NSMenu::new(*mtm);

    // Application menu.
    let app_item = item(mtm, "Slopogram", None, "");
    let app_menu = NSMenu::new(*mtm);
    app_item.setSubmenu(Some(&app_menu));
    main_menu.addItem(&app_item);

    app_menu.addItem(&item(
        mtm,
        "About Slopogram",
        Some(sel!(orderFrontStandardAboutPanel:)),
        "",
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(*mtm));
    let services_item = item(mtm, "Services", None, "");
    let services_menu = NSMenu::new(*mtm);
    services_item.setSubmenu(Some(&services_menu));
    app_menu.addItem(&services_item);
    app.setServicesMenu(Some(&services_menu));
    app_menu.addItem(&NSMenuItem::separatorItem(*mtm));
    app_menu.addItem(&item(mtm, "Hide Slopogram", Some(sel!(hide:)), "h"));
    let hide_others = item(mtm, "Hide Others", Some(sel!(hideOtherApplications:)), "h");
    hide_others
        .setKeyEquivalentModifierMask(NSEventModifierFlags::Command | NSEventModifierFlags::Option);
    app_menu.addItem(&hide_others);
    app_menu.addItem(&item(
        mtm,
        "Show All",
        Some(sel!(unhideAllApplications:)),
        "",
    ));
    app_menu.addItem(&NSMenuItem::separatorItem(*mtm));
    app_menu.addItem(&item(mtm, "Quit Slopogram", Some(sel!(terminate:)), "q"));

    // Window menu.
    let window_item = item(mtm, "Window", None, "");
    let window_menu = NSMenu::new(*mtm);
    window_item.setSubmenu(Some(&window_menu));
    main_menu.addItem(&window_item);
    app.setWindowsMenu(Some(&window_menu));

    window_menu.addItem(&item(mtm, "Minimize", Some(sel!(performMiniaturize:)), "m"));
    window_menu.addItem(&item(mtm, "Zoom", Some(sel!(performZoom:)), ""));
    window_menu.addItem(&NSMenuItem::separatorItem(*mtm));
    window_menu.addItem(&item(
        mtm,
        "Bring All to Front",
        Some(sel!(arrangeInFront:)),
        "",
    ));

    app.setMainMenu(Some(&main_menu));
}

fn item(
    mtm: &MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: all selectors above are valid AppKit first-responder selectors.
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(*mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

/// The macOS system SOCKS proxy, as a `socks5://` URL for grammers, if one is
/// enabled. Best-effort: failures fall back to a direct connection. An enabled
/// HTTP proxy is unsupported by the Telegram transport and ignored.
pub fn system_proxy() -> Option<String> {
    let output = std::process::Command::new("scutil")
        .args(["--proxy"])
        .output()
        .ok()?;
    if !output.status.success() {
        eprintln!("Slopogram: scutil --proxy failed; using a direct connection.");
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    match parse_system_proxy(&text) {
        Some(proxy) => Some(proxy),
        None => {
            if enabled(&text, "HTTPEnable") {
                eprintln!(
                    "Slopogram: the system HTTP proxy is set, but the Telegram transport only supports SOCKS5; using a direct connection."
                );
            }
            None
        }
    }
}

fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix(key)?.strip_prefix(" : "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn enabled(text: &str, key: &str) -> bool {
    value(text, key) == Some("1")
}

fn parse_system_proxy(text: &str) -> Option<String> {
    if !enabled(text, "SOCKSEnable") {
        return None;
    }
    let host = value(text, "SOCKSProxy")?;
    let port: u16 = value(text, "SOCKSPort")?.parse().ok()?;
    if port == 0 {
        return None;
    }
    // scutil emits bare IPv6 addresses; bracket them for URL syntax.
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    // scutil does not expose the proxy password (keychain); the username is
    // included so authenticated proxies at least get the account name.
    let user = value(text, "SOCKSUser")
        .map(|user| format!("{user}@"))
        .unwrap_or_default();
    Some(format!("socks5://{user}{host}:{port}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enabled_socks_proxy() {
        let text = "SOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 1080\n";
        assert_eq!(
            parse_system_proxy(text).as_deref(),
            Some("socks5://127.0.0.1:1080")
        );
    }

    #[test]
    fn includes_proxy_username_when_set() {
        let text = "SOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 7890\nSOCKSUser : sosal\n";
        assert_eq!(
            parse_system_proxy(text).as_deref(),
            Some("socks5://sosal@127.0.0.1:7890")
        );
    }

    #[test]
    fn brackets_ipv6_proxy_hosts() {
        let text = "SOCKSEnable : 1\nSOCKSProxy : ::1\nSOCKSPort : 9050\n";
        assert_eq!(
            parse_system_proxy(text).as_deref(),
            Some("socks5://[::1]:9050")
        );
    }

    #[test]
    fn disabled_proxy_is_ignored() {
        let text = "SOCKSEnable : 0\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 1080\n";
        assert_eq!(parse_system_proxy(text), None);
    }

    #[test]
    fn malformed_proxy_is_ignored() {
        assert_eq!(
            parse_system_proxy("SOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\n"),
            None
        );
        assert_eq!(
            parse_system_proxy("SOCKSEnable : 1\nSOCKSPort : 1080\n"),
            None
        );
    }

    #[test]
    fn parses_indented_scutil_output() {
        let text =
            "<dictionary> {\n  SOCKSEnable : 1\n  SOCKSProxy : 127.0.0.1\n  SOCKSPort : 7890\n}\n";
        assert_eq!(
            parse_system_proxy(text).as_deref(),
            Some("socks5://127.0.0.1:7890")
        );
    }

    #[test]
    fn rejects_zero_port() {
        assert_eq!(
            parse_system_proxy("SOCKSEnable : 1\nSOCKSProxy : 127.0.0.1\nSOCKSPort : 0\n"),
            None
        );
    }
}
