import SwiftUI
import MarginaliaUI

// `swift run MarginaliaUIPreview` launches this. It doubles as the binary
// that `build-app-bundle.sh` wraps into `Marginalia.app`, so everything
// under `.commands { … }` also powers the final .app's menu bar.

/// Shared transient UI state the Commands struct and window share
/// (SwiftUI's `.commands` can't own @State directly, so we bounce through
/// an ObservableObject).
@MainActor
final class AppUIState: ObservableObject {
    @Published var showingUrlImport: Bool = false
}

/// NSApplicationDelegate hook. Sole job: tell macOS to quit the process
/// when the last window closes (red ⊗ button), instead of keeping a
/// zombie dock icon around the way SwiftUI does by default.
final class MarginaliaAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@main
struct MarginaliaApp: App {
    @NSApplicationDelegateAdaptor(MarginaliaAppDelegate.self) private var appDelegate
    @StateObject private var host: MockHost
    @StateObject private var poller: EventPoller
    @StateObject private var uiState = AppUIState()
    @State private var onboardingStep: OnboardingStep = .none

    static let onboardedKey = "com.gibbio.marginalia.hasOnboarded"

    enum OnboardingStep { case none, welcome, installModels }

    init() {
        Fonts.registerBundled()
        let h = MockHost()
        // Auto-detect first run: if the UserDefaults flag is missing
        // (fresh install) OR the env var forces it (for design review),
        // route through the onboarding flow. The FFIHost overrides this
        // by inspecting disk (marginalia.toml + installed models).
        let hasOnboarded = UserDefaults.standard.bool(forKey: Self.onboardedKey)
        let forceOnboarding = ProcessInfo.processInfo.environment["ONBOARDING"] == "1"
        if forceOnboarding || !hasOnboarded {
            h.needsOnboarding = true
        }
        _host = StateObject(wrappedValue: h)
        // Poller drives auto-advance ticks and drains the event buffer.
        // MockHost.tick() is a no-op; FFIHost.tick() calls autoAdvance +
        // pollEvents.
        _poller = StateObject(wrappedValue: EventPoller(
            source: { [weak h] in h?.tick() ?? [] },
            sink:   { [weak h] event in h?.handle(event: event) }
        ))
    }

    var body: some Scene {
        WindowGroup("Marginalia") {
            rootView
                .environmentObject(uiState)
                .onAppear {
                    poller.start()
                    if host.needsOnboarding { onboardingStep = .welcome }
                }
                .onDisappear { poller.stop() }
                .sheet(isPresented: $uiState.showingUrlImport) {
                    UrlImportSheet(isPresented: $uiState.showingUrlImport) { url in
                        Task {
                            _ = try? await host.importUrl(url)
                            await host.refreshLibrary()
                        }
                    }
                }
        }
        // Hide the native title bar so our custom warm-dark one doesn't
        // stack on top of it. Traffic-light controls stay visible —
        // they overlay the content at the standard macOS position
        // (top-left, ~x=20 y=20), so our custom title bar leaves room
        // for them with a 82-pt leading padding.
        .windowStyle(.hiddenTitleBar)
        .commands { MarginaliaCommands(host: host, uiState: uiState) }
    }

    @ViewBuilder
    private var rootView: some View {
        switch onboardingStep {
        case .none:
            MarginaliaWindow(host: host, initialMode: .reading)
        case .welcome:
            WelcomeView(accent: .default) {
                onboardingStep = .installModels
            }
        case .installModels:
            InstallModelsView(
                accent: .default,
                assets: [
                    .init(id: "mlx-core", label: "Kokoro MLX (voce + motore)",
                          size: "310 MB", installed: false),
                    .init(id: "voice:if_sara", label: "Sara (italiano, F)",
                          size: "0,5 MB", installed: false),
                ],
                onProceed: {
                    host.markOnboardingComplete()
                    UserDefaults.standard.set(true, forKey: Self.onboardedKey)
                    onboardingStep = .none
                },
                onSkip: {
                    host.markOnboardingComplete()
                    UserDefaults.standard.set(true, forKey: Self.onboardedKey)
                    onboardingStep = .none
                }
            )
        }
    }
}

/// Menu bar + keyboard shortcuts. Injected via `.commands { }` on the
/// WindowGroup; one shared `host` binds everything.
struct MarginaliaCommands: Commands {
    @ObservedObject var host: MockHost
    @ObservedObject var uiState: AppUIState

    var body: some Commands {
        // Replace the default "File" with our own.
        CommandGroup(replacing: .newItem) {
            Button("Importa documento…") { importFile() }
                .keyboardShortcut("o", modifiers: [.command])
            Button("Importa da URL…") { uiState.showingUrlImport = true }
                .keyboardShortcut("u", modifiers: [.command])
            Divider()
            Button("Chiudi sessione") {
                Task { try? await host.stop() }
            }
            .keyboardShortcut("w", modifiers: [.command])
            .disabled(host.currentSession == nil)
        }

        // Custom "Lettura" menu grouped under .appVisibility so macOS puts
        // it between View and Help.
        CommandMenu("Lettura") {
            Button(host.currentSession?.playbackState == .playing ? "Pausa" : "Riprendi") {
                Task {
                    if host.currentSession?.playbackState == .playing {
                        try? await host.pause()
                    } else {
                        try? await host.resume()
                    }
                }
            }
            .keyboardShortcut(.space, modifiers: [])
            .disabled(host.currentSession == nil)

            Divider()
            Button("Chunk successivo") { Task { try? await host.next() } }
                .keyboardShortcut(.rightArrow, modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Chunk precedente") { Task { try? await host.back() } }
                .keyboardShortcut(.leftArrow, modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Ripeti chunk") { Task { try? await host.repeatCurrent() } }
                .keyboardShortcut("r", modifiers: [.command])
                .disabled(host.currentSession == nil)

            Divider()
            Button("Capitolo successivo") { Task { try? await host.nextChapter() } }
                .keyboardShortcut(.rightArrow, modifiers: [.command, .shift])
                .disabled(host.currentSession == nil)
            Button("Capitolo precedente") { Task { try? await host.previousChapter() } }
                .keyboardShortcut(.leftArrow, modifiers: [.command, .shift])
                .disabled(host.currentSession == nil)

            Divider()
            Button("Salva segnalibro") { Task { try? await host.bookmark() } }
                .keyboardShortcut("b", modifiers: [.command])
                .disabled(host.currentSession == nil)
            Button("Dove sono") { _ = host.announcePosition() }
                .keyboardShortcut("?", modifiers: [.command])
                .disabled(host.currentSession == nil)
        }

        // Replace the standard "Settings…" item (⌘,) so macOS shows it under
        // the app menu at the expected spot. The window intercepts the
        // notification and toggles its mode.
        CommandGroup(replacing: .appSettings) {
            Button("Impostazioni…") {
                NotificationCenter.default.post(name: .marginaliaOpenSettings, object: nil)
            }
            .keyboardShortcut(",", modifiers: [.command])
        }

        // Add "Visualizza → Log" toggle. We broadcast a Notification so the
        // WindowGroup (which owns the `logExpanded` State) flips the pane.
        CommandGroup(after: .toolbar) {
            Button("Mostra / nascondi log") {
                NotificationCenter.default.post(name: .marginaliaToggleLog, object: nil)
            }
            .keyboardShortcut("l", modifiers: [.option])
        }

        // Drop the default "Help" for now — we don't have a help bundle.
        CommandGroup(replacing: .help) { EmptyView() }
    }

    private func importFile() {
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
    }
}
