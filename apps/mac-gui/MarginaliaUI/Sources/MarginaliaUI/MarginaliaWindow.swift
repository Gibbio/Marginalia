import SwiftUI
#if canImport(AppKit)
import AppKit
import UniformTypeIdentifiers
#endif

/// Top-level window: traffic-light bar + Sidebar + Reading/Settings panel.
///
/// In the real .app this is hosted inside a SwiftUI `WindowGroup`; in the
/// preview target it's the root view. Uses a `@State` mode switcher between
/// reading and settings — the gear icon in the Sidebar toggles.
public struct MarginaliaWindow<Host: MarginaliaHost>: View {
    public enum Mode: String { case reading, settings }

    @ObservedObject private var host: Host
    @State private var mode: Mode
    @State private var accent: Accent = .default
    @State private var logExpanded: Bool = false
    @State private var showingBookmarks: Bool = false
    /// Persisted across launches. Window owns the accent so Settings +
    /// Reading share the same colour without plumbing a binding
    /// everywhere.
    @AppStorage(AppTheme.storageKey) private var themeId: String = AppTheme.default.id

    public init(host: Host, initialMode: Mode = .reading) {
        self._host = ObservedObject(initialValue: host)
        self._mode = State(initialValue: initialMode)
    }

    public var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 0) {
                Sidebar(
                    accent: accent,
                    library: host.library,
                    micLevels: host.micLevels,
                    ttsLevels: host.ttsLevels,
                    onAddDocument: handleImport,
                    onOpenDocument: handleOpenDocument,
                    onDeleteDocument: handleDeleteDocument
                )
                Divider().frame(width: 1).overlay(Tokens.line)
                Group {
                    switch mode {
                    case .reading:
                        // Only show ReadingView when there's an active session.
                        // Without this gate, ReadingView's old fallback path
                        // would surface the hardcoded mock sample (La montagna
                        // incantata) every time a live app hadn't picked a
                        // document yet — confusing on first run and after
                        // `stop_session`.
                        if host.currentSession != nil {
                            ReadingView(
                                accent: accent,
                                host: host,
                                onOpenSettings: {
                                    mode = (mode == .settings) ? .reading : .settings
                                }
                            )
                            // Coordinate space is now defined *inside* ReadingView's
                            // mainArea (on the ZStack that holds both the chunk text
                            // and the link overlay) so the two share a frame origin.
                        } else {
                            EmptyReadingState(
                                accent: accent,
                                hasLibrary: !host.library.isEmpty,
                                onImport: handleImport
                            )
                        }
                    case .settings:
                        SettingsView(
                            host: host,
                            accent: $accent,
                            onClose: { mode = .reading }
                        )
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
            LogPane(messages: host.messages, accent: accent, expanded: $logExpanded)
        }
        .background(Tokens.bg)
        .frame(minWidth: 1100, minHeight: 700)
        .overlay {
            ToastOverlay(
                toast: Binding(
                    get: { host.transientToast },
                    set: { host.transientToast = $0 }
                ),
                accent: accent
            )
        }
        .overlay {
            // Blocking ingest overlay — dims the window and runs a spinner
            // while the runtime is chunking a freshly imported document.
            // Cleared by `IngestFinished` in `FFIHost.handle(event:)`.
            if let source = host.ingestingSource {
                IngestOverlay(source: source, accent: accent)
                    .transition(.opacity)
            }
        }
        // Drop-to-import: dragging a supported document onto the window
        // triggers the same import path as `⌘O`. `Info.plist` declares
        // these types under `CFBundleDocumentTypes`; the `.onDrop` just
        // mirrors that list so the OS surfaces a copy-cursor on hover.
        .onDrop(
            of: [.pdf, .epub, .plainText, .fileURL],
            isTargeted: nil,
            perform: handleDrop
        )
        .onAppear { accent = AppTheme.accent(for: themeId) }
        .onChange(of: themeId) { _, new in
            withAnimation(.easeInOut(duration: 0.25)) {
                accent = AppTheme.accent(for: new)
            }
        }
        .onReceive(NotificationCenter.default.publisher(for: .marginaliaToggleLog)) { _ in
            logExpanded.toggle()
        }
        .onReceive(NotificationCenter.default.publisher(for: .marginaliaOpenSettings)) { _ in
            mode = (mode == .settings) ? .reading : .settings
        }
        .onReceive(NotificationCenter.default.publisher(for: .marginaliaShowBookmarks)) { _ in
            showingBookmarks = true
        }
        .sheet(isPresented: $showingBookmarks) {
            BookmarkListView(
                bookmarks: host.notes.filter { $0.isBookmark },
                accent: accent,
                onSelect: { sec, ck in
                    Task { try? await host.seekToChunk(section: sec, chunk: ck) }
                },
                onDismiss: { showingBookmarks = false }
            )
        }
    }

    private func handleImport() {
        #if canImport(AppKit)
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.canChooseFiles = true
        panel.allowedContentTypes = [.pdf, .epub, .plainText]
        if panel.runModal() == .OK, let url = panel.url {
            Task {
                _ = try? await host.importFile(url: url)
                await host.refreshLibrary()
            }
        }
        #endif
    }

    private func handleOpenDocument(_ id: String) {
        Task {
            try? await host.openDocument(id: id)
            mode = .reading
        }
    }

    /// Intercept the sidebar's delete intent with a confirmation alert
    /// before firing. Deletion cascades to notes + sessions, so making
    /// the user think twice is cheap insurance.
    private func handleDeleteDocument(_ id: String) {
        #if canImport(AppKit)
        let title = host.library.first(where: { $0.id == id })?.title ?? "questo documento"
        let alert = NSAlert()
        alert.messageText = "Rimuovere \"\(title)\"?"
        alert.informativeText = "Il documento verrà eliminato dalla libreria insieme a tutte le note e alla cache TTS. Il file originale sul disco non viene toccato."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Rimuovi")
        alert.addButton(withTitle: "Annulla")
        if alert.runModal() == .alertFirstButtonReturn {
            Task { await host.deleteDocument(id: id) }
        }
        #else
        Task { await host.deleteDocument(id: id) }
        #endif
    }

    /// Consume a drag-drop payload and import any file URLs we recognise.
    /// NSItemProvider hands us URLs asynchronously — wait for all, filter
    /// by extension, fire imports in parallel. Returns `true` iff at
    /// least one provider vended a file URL (signals the OS to accept
    /// the drop).
    private func handleDrop(providers: [NSItemProvider]) -> Bool {
        let accepted = Set(["pdf", "epub", "txt", "md", "markdown"])
        var handled = false
        for provider in providers {
            guard provider.canLoadObject(ofClass: URL.self) else { continue }
            handled = true
            _ = provider.loadObject(ofClass: URL.self) { url, _ in
                guard let url,
                      accepted.contains(url.pathExtension.lowercased())
                else { return }
                Task { @MainActor in
                    _ = try? await host.importFile(url: url)
                    await host.refreshLibrary()
                }
            }
        }
        return handled
    }

}

// Convenience for the preview target with mock host.
public extension MarginaliaWindow where Host == MockHost {
    static func mock(initialMode: Mode = .reading) -> MarginaliaWindow<MockHost> {
        MarginaliaWindow<MockHost>(host: MockHost(), initialMode: initialMode)
    }
}

public extension Notification.Name {
    /// Fired by the "Visualizza → Log" menu command to toggle the log pane.
    static let marginaliaToggleLog = Notification.Name("com.gibbio.marginalia.toggleLog")
    /// Fired by ⌘, to open/close the Settings view without clicking the gear.
    static let marginaliaOpenSettings = Notification.Name("com.gibbio.marginalia.openSettings")
    /// Fired by ⌘⌥B to open the bookmark-list sheet.
    static let marginaliaShowBookmarks = Notification.Name("com.gibbio.marginalia.showBookmarks")
}
