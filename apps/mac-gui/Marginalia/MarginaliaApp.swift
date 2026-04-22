// Marginalia — macOS app entry point (Xcode target).
//
// This file is the `@main` for the shipped `Marginalia.app`. The SwiftPM
// executable target `MarginaliaUILive` keeps a near-identical copy at
// `MarginaliaUI/Sources/MarginaliaUILive/main.swift` for the dev path
// (`swift run MarginaliaUILive`). The Xcode project (Workstream H) always
// compiles with MARGINALIA_FFI on — hence no `#if MARGINALIA_FFI` guard here.
//
// Responsibilities at launch:
//   • Register bundled fonts.
//   • Resolve the config file under Application Support; seed a minimal TOML
//     on first run.
//   • Point the AEC3 / Swift STT helper at the bundled binary in
//     `Contents/Helpers/` (set by the launcher stub in the shell bundler;
//     Xcode builds get the same via `stage-resources.sh`).
//   • Bring up FFIHost + EventPoller, or route to onboarding when the config
//     didn't exist before.

import SwiftUI
import AppKit
import MarginaliaUI

/// Transient UI state shared between `LiveMarginaliaCommands` and the root
/// window. Lives here because `.commands` can't own `@State` directly.
@MainActor
final class LiveAppUIState: ObservableObject {
    @Published var showingUrlImport: Bool = false
}

/// Quit when the last window closes — otherwise macOS leaves a zombie dock
/// icon that confuses first-time users.
final class MarginaliaAppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }
}

@main
struct MarginaliaApp: App {
    @NSApplicationDelegateAdaptor(MarginaliaAppDelegate.self) private var appDelegate
    @StateObject private var host: FFIHost
    @StateObject private var poller: EventPoller
    @StateObject private var uiState = LiveAppUIState()
    @State private var onboardingStep: OnboardingStep = .none

    enum OnboardingStep { case none, welcome, permissions, installModels }

    init() {
        Fonts.registerBundled()
        Self.pointHelperAtBundleIfAvailable()

        let (path, isFirstRun) = Self.resolveConfigPath()
        let ffi: FFIHost
        do {
            ffi = try FFIHost(configPath: path)
        } catch {
            // A runtime we can't build is a fatal state — the alternative is a
            // half-usable UI that silently fails on every action. Surface the
            // error so the user can file a bug.
            let alert = NSAlert()
            alert.messageText = "Impossibile avviare Marginalia"
            alert.informativeText = "Config path: \(path)\n\n\(error)"
            alert.alertStyle = .critical
            alert.addButton(withTitle: "Chiudi")
            alert.runModal()
            fatalError("FFIHost init failed: \(error)")
        }
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
                    installSleepObserver()
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
                // Auto-preview: the moment a voice finishes installing,
                // play a short demo so the user *hears* the app work
                // rather than just seeing a checkmark. Suppresses repeats
                // by comparing against the previous snapshot.
                playPreviewForNewlyInstalledVoice(old: oldValue, new: newValue, host: host)
            }
        }
    }

    /// `~/Library/Application Support/Marginalia/marginalia.toml`, seeded
    /// with a minimal stub on first run. The Rust side merges against its
    /// own defaults; a richer file is written back on the first
    /// `save_config()` call (after onboarding or Settings edit).
    private static func resolveConfigPath() -> (path: String, isFirstRun: Bool) {
        let fm = FileManager.default
        let support = fm.urls(for: .applicationSupportDirectory, in: .userDomainMask).first!
            .appendingPathComponent("Marginalia", isDirectory: true)
        try? fm.createDirectory(at: support, withIntermediateDirectories: true)
        let cfg = support.appendingPathComponent("marginalia.toml")
        let existed = fm.fileExists(atPath: cfg.path)
        if !existed {
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

    /// Inside a signed `.app` the STT helper lives at
    /// `Contents/Helpers/stt-helper-vN`. The Rust resolver walks up from the
    /// current executable and finds it without any env var, but setting
    /// `MARGINALIA_STT_HELPER` explicitly is a belt-and-braces guard for
    /// Xcode-run-from-DerivedData paths where the `.app` structure isn't
    /// canonical.
    /// Register the system-sleep hook so we auto-pause playback when the
    /// Mac is about to sleep. Without this, waking up drops the user
    /// into the middle of a sentence they already half-forgot. No
    /// cleanup needed — the observer dies with the process.
    private func installSleepObserver() {
        NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.willSleepNotification,
            object: nil,
            queue: .main
        ) { [weak host] _ in
            guard let h = host,
                  h.currentSession?.playbackState == .playing
            else { return }
            Task { try? await h.pause() }
        }
    }

    private static func pointHelperAtBundleIfAvailable() {
        let helpers = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers")
        guard
            let contents = try? FileManager.default.contentsOfDirectory(atPath: helpers.path),
            let helper = contents.first(where: { $0.hasPrefix("stt-helper-v") })
        else { return }
        setenv("MARGINALIA_STT_HELPER", helpers.appendingPathComponent(helper).path, 1)
    }
}

// MARK: — Install UX glue

/// Minimum required assets for the onboarding installer — the TTS core plus
/// one voice matching the user's system language. Future iterations could
/// add Whisper if the user opted out of Apple STT; for now we keep it lean.
@MainActor
private func onboardingAssets(from host: FFIHost) -> [InstallModelsView.Asset] {
    let langPrefix = Locale.current.language.languageCode?.identifier ?? "en"
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

/// Demo sentence played the first time a voice is installed during
/// onboarding. Short, neutral, chosen so non-Italian voices (when we add
/// them) can swap the text without touching the wiring.
private let onboardingDemoText = "Ciao, sono la tua voce. Sono pronta per leggere con te."

/// Watches `FFIHost.inflightDownloads` for voices transitioning to
/// `.installed` and fires a one-shot TTS preview. Called from `.onChange`
/// so SwiftUI drives the comparison instead of us tracking state manually.
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
        break  // one preview per install tick — avoid overlapping voices
    }
}
