#!/usr/bin/env python3
from pathlib import Path
import re
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
upstream = root / "native/telegram-macos"
config = upstream / "packages/ApiCredentials/Sources/ApiCredentials/Config.swift"
source = config.read_text()
if "SlopogramExternalConfiguration.values" not in source:
    baseline = subprocess.check_output(["git", "-C", str(upstream), "show", "HEAD:packages/ApiCredentials/Sources/ApiCredentials/Config.swift"], text=True)
    if source != baseline:
        raise SystemExit("Upstream configuration has existing edits; no changes applied.")
    substitutions = [
        (r'public static var apiId:Int32 \{\s*return [^\n]+\s*\}', 'public static var apiId:Int32 { Int32(SlopogramExternalConfiguration.values["TELEGRAM_API_ID"] ?? "") ?? 0 }'),
        (r'public static var apiHash:String \{\s*return [^\n]+\s*\}', 'public static var apiHash:String { SlopogramExternalConfiguration.values["TELEGRAM_API_HASH"] ?? "" }'),
        (r'public static var bundleId: String \{\s*return [^\n]+\s*\}', 'public static var bundleId: String { "one.slopogram.Slopogram.Native" }'),
        (r'public static var teamId: String \{\s*return [^\n]+\s*\}', 'public static var teamId: String { "Slopogram" }'),
        (r'public static var containerURL: URL\? \{.*?\n    \}', 'public static var containerURL: URL? { SlopogramExternalConfiguration.dataDirectory }'),
        (r'public static var legacyContainerURL: URL\? \{.*?\n    \}', 'public static var legacyContainerURL: URL? { nil }'),
    ]
    for pattern, replacement in substitutions:
        source, count = re.subn(pattern, lambda _: replacement, source, count=1, flags=re.S)
        if count != 1:
            raise SystemExit("Upstream configuration differs from expected; no changes applied.")
    config.write_text(source)
source = config.read_text()
if "public static var containerURL: URL? { SlopogramExternalConfiguration.dataDirectory }" not in source:
    source, count = re.subn(r'public static var containerURL: URL\? \{\n.*?\n    \}', lambda _: 'public static var containerURL: URL? { SlopogramExternalConfiguration.dataDirectory }', source, count=1, flags=re.S)
    if count != 1:
        raise SystemExit("Data directory configuration differs from expected; inspect before applying.")
    config.write_text(source)
shutil.copyfile(root / "native/telegram-patches/SlopogramExternalConfiguration.swift", config.parent / "SlopogramExternalConfiguration.swift")
main = upstream / "Telegram-Mac/app/main.swift"
text = main.read_text()
marker = 'if !SlopogramExternalConfiguration.valid {'
if marker not in text:
    anchor = 'private let OPENSSL_INIT_NO_ATEXIT: UInt64 = 0x00080000\n'
    if anchor not in text:
        raise SystemExit("Upstream startup differs from expected; inspect before applying.")
    text = text.replace(anchor, anchor + '''\nif !SlopogramExternalConfiguration.valid {
    let alert = NSAlert()
    alert.messageText = "Slopogram needs Telegram API credentials"
    alert.informativeText = SlopogramExternalConfiguration.error ?? "Set TELEGRAM_API_ID and TELEGRAM_API_HASH in the environment or an external .env file, then restart Slopogram."
    alert.runModal()
    exit(1)
}
''', 1)
    main.write_text(text)
text = main.read_text()
if "SlopogramProxyOverride.set" not in text:
    text = text.replace("import ApiCredentials\n", "import ApiCredentials\nimport TelegramCore\n", 1)
    anchor = '_ = OPENSSL_init_crypto(OPENSSL_INIT_NO_ATEXIT, nil)'
    if anchor not in text:
        raise SystemExit("Upstream startup differs from expected; proxy was not installed.")
    text = text.replace(anchor, """if let proxy = SlopogramExternalConfiguration.proxy {
    SlopogramProxyOverride.set(ProxyServerSettings(host: proxy.host, port: proxy.port, connection: .socks5(username: proxy.username, password: proxy.password)))
}

""" + anchor, 1)
    main.write_text(text)

text = main.read_text()
if "SlopogramStreamerMode.install(" not in text:
    anchor = '@_silgen_name("OPENSSL_init_crypto")'
    if anchor not in text:
        raise SystemExit("Upstream startup differs from expected; streamer mode was not installed.")
    text = text.replace(anchor, 'SlopogramStreamerMode.install(profileKey: SlopogramExternalConfiguration.keychainScope ?? "default")\n\n' + anchor, 1)
    main.write_text(text)
