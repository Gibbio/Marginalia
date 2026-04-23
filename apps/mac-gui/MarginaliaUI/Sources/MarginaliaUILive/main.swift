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
    @AppStorage(AppTheme.storageKey) private var themeId: String = AppTheme.default.id
    @AppStorage("marginalia.uiLocale") private var uiLocale: String = "it"

    enum OnboardingStep {
        case none, welcome, permissions, installModels, voice, uiLanguage, theme
    }

    /// Persisted across launches — `true` once the user clicks "continua"
    /// or "salta per ora" on the InstallModels step. Lives in UserDefaults
    /// (scoped by bundle id automatically). `--reset` clears this.
    static let onboardingCompleteKey = "marginalia.onboardingComplete"

    init() {
        Fonts.registerBundled()

        // `--reset` wipes config / sqlite / notes / TTS cache / mlx mirror
        // and clears the onboarding flag. HuggingFace-cached model weights
        // stay put (expensive to redownload, `is_asset_cached` picks them
        // up on the next install). TCC cannot be reset from inside the
        // app — we print the tccutil commands to stderr for the user.
        if CommandLine.arguments.contains("--reset") {
            Self.performReset()
        }

        let (path, _) = Self.resolveConfigPath()
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
        // The config file is NOT a reliable "first run" signal —
        // resolveConfigPath() seeds a stub before the user sees the
        // welcome screen. UserDefaults is durable across launches, so
        // quitting mid-onboarding resumes the flow next time.
        if !UserDefaults.standard.bool(forKey: Self.onboardingCompleteKey) {
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
            DownloadManagerView(
                accent: AppTheme.accent(for: themeId),
                installations: host.installations,
                inflightStates: host.inflightDownloads,
                onInstall: { host.installAsset($0) },
                onUninstall: { host.uninstallAsset($0) },
                onProceed: { onboardingStep = .voice },
                onSkip: { onboardingStep = .voice }
            )
            .onAppear { Task { await host.refreshInstallations() } }
            .onChange(of: host.inflightDownloads) { oldValue, newValue in
                playPreviewForNewlyInstalledVoice(old: oldValue, new: newValue, host: host)
            }

        case .voice:
            VoiceSelectView(
                accent: AppTheme.accent(for: themeId),
                voices: host.installations.filter { $0.category == "voice" && $0.installed },
                selectedId: host.currentSpec.voice,
                onSelect: { voiceId in
                    // Persist the pick: build a new ProviderSpec with the
                    // selected voice and apply. Keeps the TTS backend /
                    // language / STT engine untouched.
                    let spec = ProviderSpec(
                        ttsBackend: host.currentSpec.ttsBackend,
                        voice: voiceId,
                        sttEngine: host.currentSpec.sttEngine,
                        language: host.currentSpec.language
                    )
                    Task { _ = try? await host.apply(spec: spec) }
                },
                onProceed: { onboardingStep = .uiLanguage },
                onBack: { onboardingStep = .installModels }
            )

        case .uiLanguage:
            UILanguageSelectView(
                accent: AppTheme.accent(for: themeId),
                selectedLocale: uiLocale,
                onSelect: { uiLocale = $0 },
                onProceed: { onboardingStep = .theme },
                onBack: { onboardingStep = .voice }
            )

        case .theme:
            ThemeSelectView(
                accent: AppTheme.accent(for: themeId),
                selectedThemeId: themeId,
                onSelect: { themeId = $0 },
                onProceed: {
                    host.markOnboardingComplete()
                    onboardingStep = .none
                },
                onBack: { onboardingStep = .uiLanguage }
            )
        }
    }

    /// Resolves `~/Library/Application Support/Marginalia/marginalia.toml`.
    /// Creates the directory and seeds a minimal TOML on first run (the
    /// Rust side fills in its own defaults). The returned `isFirstRun`
    /// flag is based on prior existence of the config file — kept for
    /// callers who want it, but onboarding uses UserDefaults instead
    /// (more reliable — this path is written before the user interacts).
    private static func resolveConfigPath() -> (path: String, isFirstRun: Bool) {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("Marginalia", isDirectory: true)
        try? fm.createDirectory(at: support, withIntermediateDirectories: true)
        let cfg = support.appendingPathComponent("marginalia.toml")
        let existed = fm.fileExists(atPath: cfg.path)
        if !existed {
            // Absolute path for `[mlx] model` so Discovery::list_mlx_voices
            // has a real directory to scan — the stock default is a HF
            // repo id, not a filesystem path. install_asset mirrors
            // downloaded weights into this dir so the voice picker
            // populates as soon as onboarding finishes.
            let modelsDir = support.appendingPathComponent("models/mlx", isDirectory: true)
            try? fm.createDirectory(at: modelsDir, withIntermediateDirectories: true)
            let seed = """
            # Marginalia — config seed (first run, \(ISO8601DateFormatter().string(from: Date())))
            # The app rewrites this file via save_config() once settings change.

            [mlx]
            model = "\(modelsDir.path)"
            voice = "if_sara"
            """
            try? seed.write(to: cfg, atomically: true, encoding: .utf8)
        }
        return (cfg.path, !existed)
    }

    /// Wipe the Application Support directory so the next launch behaves
    /// like a fresh install. The HuggingFace model cache is NOT touched
    /// — weights are expensive to redownload and `is_asset_cached` picks
    /// them up transparently. TCC cannot be reset from inside the app
    /// (sandboxed or not, it's a privileged system service), so we print
    /// the tccutil incantation to stderr for the user.
    ///
    /// Pass via: `open Marginalia.app --args --reset`
    /// Or: `make reset-gui` (also revokes TCC).
    private static func performReset() {
        let fm = FileManager.default
        guard let support = fm.urls(for: .applicationSupportDirectory,
                                     in: .userDomainMask).first?
            .appendingPathComponent("Marginalia", isDirectory: true),
              fm.fileExists(atPath: support.path)
        else {
            FileHandle.standardError.write(Data("[reset] no support directory to wipe\n".utf8))
            // Still clear the flag — user explicitly asked for reset.
            UserDefaults.standard.removeObject(forKey: Self.onboardingCompleteKey)
            return
        }
        do {
            try fm.removeItem(at: support)
            FileHandle.standardError.write(Data(
                "[reset] wiped \(support.path)\n".utf8
            ))
        } catch {
            FileHandle.standardError.write(Data(
                "[reset] failed to wipe \(support.path): \(error)\n".utf8
            ))
            return
        }
        // Nuke the whole preferences domain — covers the onboarding
        // flag, theme, custom hue, UI locale, and window frames.
        // `synchronize()` forces the plist flush so a sibling `open`
        // launched immediately after sees a clean state.
        if let bundleId = Bundle.main.bundleIdentifier {
            UserDefaults.standard.removePersistentDomain(forName: bundleId)
            UserDefaults.standard.synchronize()
        } else {
            UserDefaults.standard.removeObject(forKey: Self.onboardingCompleteKey)
        }
        FileHandle.standardError.write(Data("[reset] cleared UserDefaults\n".utf8))

        let bundleId = Bundle.main.bundleIdentifier ?? "com.gibbio.marginalia.dev"
        FileHandle.standardError.write(Data(
            """
            [reset] to also revoke microphone + speech-recognition prompts:
                tccutil reset Microphone \(bundleId)
                tccutil reset SpeechRecognition \(bundleId)
            (or all: tccutil reset All \(bundleId))

            """.utf8
        ))
    }
}

// MARK: — Install UX glue

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
