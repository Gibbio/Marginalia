// `swift run MarginaliaUILive` launches this. It doubles as the binary that
// `build-app-bundle.sh --live` wraps into the shipped `Marginalia.app`. Mirrors
// `MarginaliaUIPreview/main.swift` but swaps `MockHost` → `FFIHost` and adds
// config-path resolution + onboarding detection driven by the real disk state.
//
// Gated on `MARGINALIA_FFI`, which Package.swift defines only for this target
// (and for MarginaliaUI) when `MARGINALIA_LIVE=1` is set at build time.

#if MARGINALIA_FFI
import SwiftUI
import AppKit
import MarginaliaUI

/// Shared transient UI state the Commands struct and window share. Lives in
/// the live executable target because SwiftUI's `.commands` can't own @State
/// directly.
@MainActor
final class LiveAppUIState: ObservableObject {
    @Published var showingUrlImport: Bool = false
}

/// Same job as the preview delegate: terminate when the last window closes,
/// instead of leaving a zombie dock icon.
final class MarginaliaAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@main
struct MarginaliaLiveApp: App {
    @NSApplicationDelegateAdaptor(MarginaliaAppDelegate.self) private var appDelegate
    @StateObject private var host: FFIHost
    @StateObject private var poller: EventPoller
    @StateObject private var uiState = LiveAppUIState()
    @State private var onboardingStep: OnboardingStep = .none

    enum OnboardingStep { case none, welcome, permissions, installModels }

    init() {
        Fonts.registerBundled()
        let (path, isFirstRun) = Self.resolveConfigPath()
        let ffi: FFIHost
        do {
            ffi = try FFIHost(configPath: path)
        } catch {
            // Hard-fail at launch — the alternative is a half-usable app
            // without a runtime, which would silently confuse the user. The
            // NSAlert surfaces the actual error so they can file a bug.
            let alert = NSAlert()
            alert.messageText = "Impossibile avviare Marginalia"
            alert.informativeText = "Config path: \(path)\n\n\(error)"
            alert.alertStyle = .critical
            alert.addButton(withTitle: "Chiudi")
            alert.runModal()
            fatalError("FFIHost init failed: \(error)")
        }
        // Seed the onboarding banner if this was a fresh install (no config
        // existed before). The FFIHost itself also maintains a
        // `needsOnboarding` flag that can refine this — e.g. config exists
        // but no models are installed — which we'll honor in onAppear.
        if isFirstRun {
            ffi.markNeedsOnboarding()
        }
        _host = StateObject(wrappedValue: ffi)
        _poller = StateObject(wrappedValue: EventPoller(
            source: { [weak ffi] in ffi?.tick() ?? [] },
            sink:   { [weak ffi] event in ffi?.handle(event: event) }
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
        .windowStyle(.hiddenTitleBar)
        .commands { LiveMarginaliaCommands(host: host, uiState: uiState) }
    }

    @ViewBuilder
    private var rootView: some View {
        switch onboardingStep {
        case .none:
            MarginaliaWindow(host: host, initialMode: .reading)
        case .welcome:
            WelcomeView(accent: .default) { onboardingStep = .permissions }
        case .permissions:
            PermissionsCheckView(
                accent: .default,
                onContinue: { onboardingStep = .installModels },
                onBack: { onboardingStep = .welcome }
            )
        case .installModels:
            InstallModelsView(
                accent: .default,
                assets: onboardingAssets(from: host),
                inflightStates: host.inflightDownloads,
                onInstall: { host.installAsset($0) },
                onProceed: {
                    host.markOnboardingComplete()
                    onboardingStep = .none
                },
                onSkip: {
                    host.markOnboardingComplete()
                    onboardingStep = .none
                }
            )
            .onAppear { Task { await host.refreshInstallations() } }
            .onChange(of: host.inflightDownloads) { oldValue, newValue in
                playPreviewForNewlyInstalledVoice(old: oldValue, new: newValue, host: host)
            }
        }
    }

    /// Resolves `~/Library/Application Support/Marginalia/marginalia.toml`.
    /// Creates the directory and seeds a minimal TOML on first run (the
    /// Rust side fills in its own defaults). Returns the path plus a flag so
    /// the caller can route to onboarding when this was a fresh install.
    private static func resolveConfigPath() -> (path: String, isFirstRun: Bool) {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("Marginalia", isDirectory: true)
        try? fm.createDirectory(at: support, withIntermediateDirectories: true)
        let cfg = support.appendingPathComponent("marginalia.toml")
        let existed = fm.fileExists(atPath: cfg.path)
        if !existed {
            // Minimal seed. `marginalia-config::load_from` will merge against
            // its defaults; a richer file gets written once the user
            // completes onboarding (voice choice, language, etc.).
            let seed = """
            # Marginalia — config seed (first run, \(ISO8601DateFormatter().string(from: Date())))
            # The app rewrites this file via save_config() once settings change.

            [tts]
            voice = "if_sara"
            """
            try? seed.write(to: cfg, atomically: true, encoding: .utf8)
        }
        return (cfg.path, !existed)
    }
}

// MARK: — Install UX glue

/// Required assets shown in the onboarding installer. We want the smallest
/// workable set: the MLX core + one voice matching the user's language.
///
/// Why this is Live-only: `InstallableAsset` (mapped from FFI) carries
/// the `id`/`label` shape we need, and we're in the MARGINALIA_FFI target
/// by construction here.
@MainActor
private func onboardingAssets(from host: FFIHost) -> [InstallModelsView.Asset] {
    let langPrefix = Locale.current.language.languageCode?.identifier ?? "en"
    // Voice ids map language to letter: i = it, a = en-US, j = ja, etc.
    // Pick a default voice id for the detected language — fall back to
    // `af_bella` (English) if we don't have a curated pick.
    let defaultVoice: String
    switch langPrefix {
    case "it": defaultVoice = "voice:if_sara"
    case "en": defaultVoice = "voice:af_bella"
    default:   defaultVoice = "voice:af_bella"
    }

    let required = ["mlx-core", defaultVoice]
    return host.installations
        .filter { required.contains($0.id) }
        .map {
            InstallModelsView.Asset(
                id: $0.id, label: $0.label, size: $0.size, installed: $0.installed
            )
        }
}

/// Demo sentence auto-played when a voice finishes installing during
/// onboarding. Short, neutral — lets the user immediately hear that the
/// app works instead of staring at a checkmark.
private let onboardingDemoText = "Ciao, sono la tua voce. Sono pronta per leggere con te."

/// Fires a one-shot TTS preview when a voice transitions to `.installed`.
/// Driven by `.onChange(of: host.inflightDownloads)` so SwiftUI owns the
/// old/new diff rather than us tracking it manually.
@MainActor
private func playPreviewForNewlyInstalledVoice(
    old: [String: InstallUiState],
    new: [String: InstallUiState],
    host: FFIHost
) {
    for (id, state) in new
        where state == .installed
        && old[id] != .installed
        && id.hasPrefix("voice:")
    {
        let voiceId = String(id.dropFirst("voice:".count))
        Task {
            guard let path = try? await host.synthesizePreview(
                text: onboardingDemoText, voice: voiceId
            ), !path.isEmpty else { return }
            await MainActor.run { PreviewSoundCache.play(path: path) }
        }
        break
    }
}

/// Translate `FFIHost.InstallState` → `InstallModelsView.RowState`. Kept as
/// a small function rather than extending `RowState` so the mock preview
/// target (which has no FFIHost) doesn't have to compile against it.

/// Menu bar + keyboard shortcuts for the live target. Mirrors
/// `MarginaliaCommands` in the preview target — keep them in sync. If the
/// divergence becomes a maintenance cost, generify `MarginaliaCommands<Host>`
/// and move it into the `MarginaliaUI` library.
struct LiveMarginaliaCommands: Commands {
    @ObservedObject var host: FFIHost
    @ObservedObject var uiState: LiveAppUIState

    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("Importa documento…") { importFile() }
                .keyboardShortcut("o", modifiers: [.command])
            Button("Importa da URL…") { uiState.showingUrlImport = true }
                .keyboardShortcut("u", modifiers: [.command])
            Divider()
            Button("Chiudi sessione") { Task { try? await host.stop() } }
                .keyboardShortcut("w", modifiers: [.command])
                .disabled(host.currentSession == nil)
        }

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

        CommandGroup(replacing: .appSettings) {
            Button("Impostazioni…") {
                NotificationCenter.default.post(name: .marginaliaOpenSettings, object: nil)
            }
            .keyboardShortcut(",", modifiers: [.command])
        }

        CommandGroup(after: .toolbar) {
            Button("Mostra / nascondi log") {
                NotificationCenter.default.post(name: .marginaliaToggleLog, object: nil)
            }
            .keyboardShortcut("l", modifiers: [.option])
        }

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

#endif // MARGINALIA_FFI
