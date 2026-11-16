import SwiftUI
import SlopogramState

struct ContentView: View {
    @ObservedObject var store: Store
    @Environment(\.scenePhase) private var scenePhase
    @State private var phone = ""
    @State private var code = ""
    @State private var password = ""
    @State private var search = ""

    var body: some View {
        Group {
            switch store.screen {
            case .starting:
                if let setup = store.setupMessage {
                    ContentUnavailableView("Set up Slopogram", systemImage: "gearshape", description: Text(setup))
                } else if !store.connected {
                    VStack(spacing: 12) { Text("Connection interrupted"); Button("Reconnect") { store.reconnect() }.disabled(store.isReconnecting) }
                } else { ProgressView("Connecting…") }
            case .chats: conversations
            default: login
            }
        }
        .frame(minWidth: 760, minHeight: 520)
        .alert("Slopogram", isPresented: Binding(get: { store.error != nil }, set: { if !$0 { store.error = nil } })) {
            Button("OK") { store.error = nil }
        } message: { Text(store.error ?? "") }
        .onChange(of: scenePhase, initial: true) { _, phase in store.focused = phase == .active; if store.focused { store.markRead() } }
    }
    var login: some View {
        VStack(spacing: 18) {
            Image(systemName: "paperplane.circle.fill").font(.system(size: 54)).foregroundStyle(.blue)
            Text("Sign in to Telegram").font(.title2.weight(.semibold))
            switch store.screen {
            case .phone:
                Text("Enter your phone number with its country code.").foregroundStyle(.secondary)
                TextField("Phone number", text: $phone).textContentType(.telephoneNumber).onSubmit { requestCode() }
                Button("Continue", action: requestCode).buttonStyle(.borderedProminent)
                Button("Sign in with QR code") { store.command { try await $0.requestQrLogin() } }
            case .code:
                Text("Enter the code sent by Telegram.").foregroundStyle(.secondary)
                TextField("Login code", text: $code).onSubmit { submitCode() }
                Button("Continue", action: submitCode).buttonStyle(.borderedProminent)
                back
            case .password(let hint):
                Text(hint.isEmpty ? "Enter your two-step verification password." : hint).foregroundStyle(.secondary)
                SecureField("Password", text: $password).onSubmit { submitPassword() }
                Button("Continue", action: submitPassword).buttonStyle(.borderedProminent)
                back
            case .qr(let url):
                Text("Scan this code from Telegram on your phone.").foregroundStyle(.secondary)
                QRImage(value: url)
                Button("Use phone number") { store.command { try await $0.resetLogin() } }
            default: EmptyView()
            }
        }.frame(width: 320).textFieldStyle(.roundedBorder).padding(32)
    }
    var back: some View { Button("Back") { code = ""; password = ""; store.command { try await $0.resetLogin() } } }
    func requestCode() { let value = phone; store.command { try await $0.requestCode(phone: value) } }
    func submitCode() { let value = code; code = ""; store.command { try await $0.submitCode(code: value) } }
    func submitPassword() { let value = password; password = ""; store.command { try await $0.submitPassword(password: value) } }
    var conversations: some View {
        NavigationSplitView {
            List(selection: Binding(get: { store.selected }, set: { if let id = $0 { store.select(id) } })) {
                ForEach(store.chats.filter { search.isEmpty || $0.title.localizedCaseInsensitiveContains(search) }, id: \.id) { chat in
                    ConversationRow(title: chat.title, preview: chat.preview).tag(chat.id)
                }
            }.searchable(text: $search, prompt: "Search conversations")
                .navigationTitle("Chats")
                .navigationSplitViewColumnWidth(min: 240, ideal: 300, max: 380)
        } detail: {
            if let id = store.selected {
                VStack(spacing: 0) {
                    if !store.connected { HStack { Text("Connection interrupted"); Button("Reconnect") { store.reconnect() }.disabled(store.isReconnecting) }.font(.caption).padding(6) }
                    MessageList(messages: (store.timelines[id] ?? [:]).values.sorted { $0.id < $1.id }).id(id)
                    ForEach(store.pending.keys.sorted(), id: \.self) { key in
                        if let item = store.pending[key], item.0 == id {
                            HStack { Spacer(); Text(item.1).lineLimit(1); Text("Sending…").foregroundStyle(.secondary) }
                                .font(.caption).padding(.horizontal, 20).padding(.bottom, 6)
                        }
                    }
                    ForEach(store.failed.keys.sorted(), id: \.self) { key in
                        if let item = store.failed[key], item.0 == id {
                            HStack { Text(item.1).lineLimit(1); Spacer(); Button("Restore unconfirmed message") { store.recover(key) } }
                                .font(.caption).padding(.horizontal, 20).padding(.bottom, 6)
                        }
                    }
                    Divider()
                    HStack(alignment: .bottom, spacing: 10) {
                        Composer(text: Binding(get: { store.drafts[id] ?? "" }, set: { store.drafts[id] = $0 }), send: store.send)
                            .frame(height: 48).modifier(ComposerSurface())
                        Button(action: store.send) { Image(systemName: "arrow.up.circle.fill").font(.title2) }.buttonStyle(.plain).foregroundStyle(.blue).help("Send message")
                    }.padding(14)
                }.navigationTitle(store.chats.first(where: { $0.id == id })?.title ?? "Conversation")
                    .onAppear { store.markRead() }
            } else {
                ContentUnavailableView("Select a conversation", systemImage: "bubble.left.and.bubble.right", description: Text("Choose a chat to start messaging."))
            }
        }
    }
}
