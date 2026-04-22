// Bridge between the Swift UI layer and the real Rust runtime via
// MarginaliaKit.xcframework (produced by apps/mac-gui/scripts/build-rust-xcframework.sh).
//
// This file is gated on the `MARGINALIA_FFI` compile flag, which is only
// defined in the Xcode .app target (where MarginaliaKit.xcframework is
// linked). The Swift Package build doesn't set it, so the preview target
// runs on `MockHost` without depending on the Rust side.
//
// How to wire it up in the Xcode .app target:
// 1. Run `apps/mac-gui/scripts/build-rust-xcframework.sh`
// 2. Drag `apps/mac-gui/Generated/MarginaliaKit.xcframework` into Xcode.
// 3. Add `apps/mac-gui/Generated/Marginalia.swift` to the target.
// 4. In Build Settings → "Other Swift Flags", add `-D MARGINALIA_FFI`.
// 5. Instantiate `FFIHost(configPath: …)` at app launch and pass it to
//    `MarginaliaWindow(host:)`.

#if MARGINALIA_FFI
import Foundation
import MarginaliaKit

@MainActor
public final class FFIHost: MarginaliaHost, ObservableObject {
    private let runtime: MarginaliaKit.FfiRuntime

    @Published public private(set) var currentSpec: ProviderSpec
    @Published public private(set) var languages: [LangInfo] = []
    @Published public private(set) var voices: [VoiceInfo] = []
    @Published public private(set) var sttEngines: [SttEngineInfo] = []
    @Published public private(set) var ttsBackends: [TtsBackendInfo] = []
    @Published public var voiceCommands: [VoiceCommand] = []
    @Published public var installations: [InstallableAsset] = []
    @Published public var chunkTargetChars: Int = 300
    @Published public var sttDebug: Bool = true
    @Published public private(set) var needsOnboarding: Bool = false
    @Published public var messages: [String] = []
    @Published public var transientToast: ToastMessage? = nil
    @Published public var micLevels: [Float] = []
    @Published public var ttsLevels: [Float] = []

    public func markOnboardingComplete() {
        needsOnboarding = false
    }

    /// Set the onboarding flag after construction. The live `@main` uses
    /// this when it detects a fresh install (no `marginalia.toml` on disk
    /// before `FFIHost.init` seeded the default). Kept as an explicit method
    /// rather than a public setter so the semantic — "only flip to true, not
    /// back to false" — stays visible in grep.
    public func markNeedsOnboarding() {
        needsOnboarding = true
    }

    public func pushMessage(_ m: String) {
        if messages.count >= 64 { messages.removeFirst() }
        messages.append(m)
        if m.hasPrefix("Errore") {
            transientToast = ToastMessage(text: m, kind: .error)
        }
    }

    // Reader-side state (Workstream K).
    @Published public private(set) var library: [LibraryEntry] = []
    @Published public private(set) var currentSession: SessionState? = nil
    @Published public private(set) var currentDocument: DocumentDoc? = nil
    @Published public private(set) var notes: [MarginNote] = []
    @Published public var liveNote: MarginNote? = nil

    /// Non-nil while the TTS backend is busy synthesizing a chunk (between
    /// `SynthesisStarted` and `SynthesisReady` events). The Toolbar reads
    /// this to render the "sintetizzando…" indicator. Value is the chunk
    /// anchor — useful for future per-chunk highlighting.
    @Published public var synthesizingAnchor: String? = nil
    @Published public var ingestingSource: String? = nil

    public init(configPath: String) throws {
        self.runtime = try MarginaliaKit.FfiRuntime(configPath: configPath)
        let ff = runtime.currentSpec()
        self.currentSpec = ProviderSpec(
            ttsBackend: ff.ttsBackend, voice: ff.voice,
            sttEngine: ff.sttEngine, language: ff.language
        )
        refreshDiscovery()
        Task { [weak self] in
            await self?.refreshLibrary()
            // Session restore at launch — the runtime persists the active
            // session in SQLite, so "reopen the app" picks up exactly where
            // the user left off (chunk position + playback state). If nothing
            // to restore, this is a cheap no-op.
            if let restored = try? self?.runtime.restoreSession(), restored == true {
                await self?.refreshSessionSnapshot()
                if let docId = await self?.currentSession?.documentId {
                    await self?.refreshDocumentView(id: docId)
                    await self?.refreshNotes(documentId: docId)
                }
                // Log line + visible toast — without the toast the user
                // has no idea why the reader opened on a specific chunk
                // (the restore is silent-by-design on the runtime side).
                await MainActor.run { [weak self] in
                    guard let self = self else { return }
                    self.pushMessage("Sessione ripresa.")
                    if let s = self.currentSession {
                        self.transientToast = ToastMessage(
                            text: "Riprendo: \(s.documentTitle) · capitolo \(s.sectionIndex + 1)",
                            kind: .info
                        )
                    }
                }
            } else {
                await self?.refreshSessionSnapshot()
            }
        }
    }

    private func refreshDiscovery() {
        let allVoices = runtime.listVoices(backend: "mlx")
        self.languages = runtime.listLanguages().map { lang in
            LangInfo(bcp47: lang.bcp47, display: lang.display,
                     voiceCount: allVoices.filter { $0.lang == lang.bcp47 }.count)
        }
        self.voices = runtime.listVoices(backend: "mlx").map { v -> VoiceInfo in
            let gender: Gender
            switch v.gender {
            case .female: gender = .female
            case .male:   gender = .male
            case .unknown: gender = .unknown
            }
            return VoiceInfo(
                id: v.id, display: v.display, lang: v.lang,
                gender: gender, backend: v.backend,
                installed: true  // FFI already filters to files on disk
            )
        }
        self.sttEngines = runtime.listSttEngines().map {
            SttEngineInfo(engineId: $0.id, name: $0.name,
                          available: $0.available, reason: $0.reason,
                          note: $0.reason ?? "")
        }
        self.ttsBackends = runtime.listTtsBackends().map {
            TtsBackendInfo(backendId: $0.id, name: $0.name,
                           sub: "", available: $0.available, reason: $0.reason)
        }
    }

    public func apply(spec: ProviderSpec) async throws -> ApplyReport {
        let ffiSpec = MarginaliaKit.ProviderSpec(
            ttsBackend: spec.ttsBackend, voice: spec.voice,
            sttEngine: spec.sttEngine, language: spec.language
        )
        let report = try await Task.detached {
            try self.runtime.applyProviderSpec(spec: ffiSpec)
        }.value
        await MainActor.run {
            self.currentSpec = spec
            self.refreshDiscovery()
        }
        return ApplyReport(
            ttsSwapped: report.ttsSwapped,
            sttSwapped: report.sttSwapped,
            languageChanged: report.languageChanged,
            elapsedMs: report.elapsedMs
        )
    }

    public func saveVoiceCommands(_ commands: [VoiceCommand]) {
        self.voiceCommands = commands
        self.persist()
    }

    public func saveAudioPrefs(chunkTargetChars: Int, sttDebug: Bool) {
        self.chunkTargetChars = chunkTargetChars
        self.sttDebug = sttDebug
        self.persist()
    }

    /// Serialize the current staging state (voice commands, chunk size,
    /// STT debug) back to `marginalia.toml`. Provider settings are persisted
    /// separately by `apply(spec:)`. Failures are logged; the UI state is
    /// authoritative regardless.
    private func persist() {
        let entries = voiceCommands.map {
            MarginaliaKit.VoiceCommandEntry(action: $0.action, triggers: $0.triggers)
        }
        let chunk = UInt32(max(0, chunkTargetChars))
        let debug = sttDebug
        Task.detached { [runtime] in
            do {
                try runtime.saveConfig(voiceCommands: entries,
                                       chunkTargetChars: chunk,
                                       sttDebug: debug)
            } catch {
                // Non-fatal — log and keep UI state.
                print("[Marginalia] saveConfig failed: \(error)")
            }
        }
    }

    // MARK: — Reader-side methods (Workstream K)

    public func refreshLibrary() async {
        let docs = runtime.listDocuments()
        await MainActor.run {
            self.library = docs.map { d in
                LibraryEntry(
                    id: d.id, title: d.title,
                    subtitle: "",  // FFI DocumentListItem doesn't carry author/subtitle today
                    progressPct: 0, notes: 0, active: false
                )
            }
        }
    }

    public func openDocument(id: String) async throws {
        try runtime.startSession(documentId: id)
        await refreshSessionSnapshot()
        await refreshDocumentView(id: id)
        await refreshNotes(documentId: id)
    }

    public func importFile(url: URL) async throws -> String {
        let result = try runtime.ingestFile(path: url.path)
        await refreshLibrary()
        pushMessage("Importato: \(result.title)")
        return result.documentId
    }

    public func importUrl(_ url: String) async throws -> String {
        let result = try runtime.ingestUrl(url: url)
        await refreshLibrary()
        pushMessage("URL: \(result.title)")
        return result.documentId
    }

    public func bookmark() async throws {
        guard let s = currentSession else {
            pushMessage("Nessuna sessione attiva.")
            return
        }
        let label = "[BOOKMARK] \(s.sectionTitle), ch.\(s.sectionIndex + 1) chunk\(s.chunkIndex + 1)"
        _ = try runtime.createNote(text: label)
        await refreshNotes(documentId: s.documentId)
        pushMessage("Segnalibro salvato · ch.\(s.sectionIndex + 1) chunk \(s.chunkIndex + 1)")
    }

    public func announcePosition() -> String? {
        guard let s = currentSession else {
            pushMessage("Nessuna sessione attiva.")
            return nil
        }
        let pos = "capitolo \(s.sectionIndex + 1)/\(s.sectionCount) (\(s.sectionTitle)), chunk \(s.chunkIndex + 1)"
        pushMessage("Posizione: \(pos)")
        return pos
    }

    public func doctorReportJson() -> String {
        runtime.doctorReportJson()
    }

    public var whisperModelPath: String? {
        let p = runtime.whisperModelPath()
        return p.isEmpty ? nil : p
    }

    public func synthesizePreview(text: String, voice: String) async throws -> String {
        let lang = currentSpec.language.prefix(2).lowercased()
        return try await Task.detached { [runtime] in
            try runtime.synthesizePreview(text: text, voice: voice, language: String(lang))
        }.value
    }

    public func tick() -> [MarginaliaEvent] {
        // Fire-and-forget auto-advance; its side effects (chunk index bump,
        // synthesis start) arrive back through the event stream.
        _ = runtime.autoAdvance()
        // Also refresh the waveform so the Sidebar footer shows live mic/TTS.
        let snap = runtime.pollWaveform()
        micLevels = snap.micLevels
        ttsLevels = snap.ttsLevels
        return pollEvents()
    }

    public func pause() async throws { try runtime.pauseSession(); await refreshSessionSnapshot() }
    public func resume() async throws { try runtime.resumeSession(); await refreshSessionSnapshot() }
    public func stop() async throws { try runtime.stopSession(); await refreshSessionSnapshot() }
    public func next() async throws { try runtime.nextChunk(); await refreshSessionSnapshot() }
    public func back() async throws { try runtime.previousChunk(); await refreshSessionSnapshot() }
    public func repeatCurrent() async throws { try runtime.repeatChunk(); await refreshSessionSnapshot() }
    public func nextChapter() async throws { try runtime.nextChapter(); await refreshSessionSnapshot() }
    public func previousChapter() async throws { try runtime.previousChapter(); await refreshSessionSnapshot() }
    public func seekToChunk(section: Int, chunk: Int) async throws {
        try runtime.seekToChunk(sectionIndex: UInt32(max(0, section)),
                                 chunkIndex: UInt32(max(0, chunk)))
        await refreshSessionSnapshot()
    }

    public func handle(event: MarginaliaEvent) {
        switch event {
        case .synthesisStarted(_, let sec, let ck):
            synthesizingAnchor = "c\(sec)-\(ck)"
        case .synthesisReady:
            synthesizingAnchor = nil
            Task { await refreshSessionSnapshot() }
        case .chunkAdvanced, .playbackFinished,
             .sessionRestored, .sessionStopped:
            Task { await refreshSessionSnapshot() }
        case .commandRecognized(let raw, let action):
            if sttDebug { pushMessage("stt: \"\(raw)\"") }
            guard let action else { break }
            dispatchVoiceAction(action)
        case .ingestStarted(let source):
            // `source` may be a full URL or a filename — pick the short
            // form for display. File URLs use lastPathComponent; web URLs
            // fall back to the host.
            let label: String = {
                if let url = URL(string: source), url.scheme != nil {
                    return url.host ?? url.lastPathComponent
                }
                return source
            }()
            ingestingSource = label
        case .ingestFinished(_, _, let err):
            ingestingSource = nil
            if let err {
                pushMessage("Errore import: \(err)")
                transientToast = ToastMessage(
                    text: "Import fallito: \(err)", kind: .error
                )
            } else {
                Task { await refreshLibrary() }
            }
        case .runtimeError(let msg):
            pushMessage("Errore: \(msg)")
        }
    }

    /// Map a recognized voice command to the right host method. Mirrors the
    /// TUI's `resolve_action` dispatch in `apps/tui-rs/src/app.rs:717`.
    private func dispatchVoiceAction(_ action: String) {
        pushMessage("→ \(action)")
        Task { [weak self] in
            guard let self else { return }
            switch action {
            case "pause":        try? await self.pause()
            case "resume":       try? await self.resume()
            case "next":         try? await self.next()
            case "back":         try? await self.back()
            case "repeat":       try? await self.repeatCurrent()
            case "stop":         try? await self.stop()
            case "next_chapter": try? await self.nextChapter()
            case "prev_chapter": try? await self.previousChapter()
            case "bookmark":     try? await self.bookmark()
            case "where":        _ = await MainActor.run { self.announcePosition() }
            case "note":
                // Voice-triggered dictation isn't implemented in the runtime
                // yet (the TUI stubs this too). Show a placeholder live-note
                // card so the UX is coherent when the pipeline lands.
                await MainActor.run {
                    self.liveNote = MarginNote(
                        id: "live", chunkId: self.currentSession?.anchor ?? "",
                        when: "ora", quote: "", body: "…",
                        duration: "0:00", status: "", live: true
                    )
                }
            default:
                self.pushMessage("Azione sconosciuta: \(action)")
            }
        }
    }

    // MARK: — helpers

    private func refreshSessionSnapshot() async {
        let snap = runtime.sessionSnapshot()
        await MainActor.run {
            self.currentSession = snap.map { s in
                SessionState(
                    sessionId: s.sessionId, documentId: s.documentId,
                    documentTitle: "", // fill via separate lookup if needed
                    sectionIndex: Int(s.sectionIndex), sectionCount: Int(s.sectionCount),
                    sectionTitle: s.sectionTitle,
                    chunkIndex: Int(s.chunkIndex), chunkText: s.chunkText,
                    anchor: s.anchor,
                    playbackState: mapPlaybackState(s.playbackState),
                    notesCount: Int(s.notesCount),
                    voice: s.voice
                )
            }
        }
    }

    private func refreshDocumentView(id: String) async {
        let view = runtime.documentView(documentId: id)
        await MainActor.run {
            self.currentDocument = view.map { v in
                DocumentDoc(
                    documentId: v.documentId,
                    title: v.title,
                    sections: v.sections.map { s in
                        SectionDoc(
                            index: Int(s.index),
                            title: s.title,
                            chunks: s.chunks.map { c in
                                ReadingChunk(id: c.anchor, index: Int(c.index), text: c.text)
                            }
                        )
                    }
                )
            }
        }
    }

    private func refreshNotes(documentId: String) async {
        let list = runtime.listNotes(documentId: documentId)
        await MainActor.run {
            self.notes = list.map { n in
                MarginNote(
                    id: n.noteId, chunkId: n.anchor,
                    when: String(n.createdAtIso.prefix(10)),
                    quote: "", body: n.text,
                    duration: "", status: "", live: false
                )
            }
        }
    }

    private func mapPlaybackState(_ p: MarginaliaKit.PlaybackState) -> PlaybackState {
        switch p {
        case .idle: return .idle
        case .playing: return .playing
        case .paused: return .paused
        case .finished: return .finished
        case .unknown: return .unknown
        }
    }

    /// Drain event buffer — wired into `EventPoller.source`.
    public func pollEvents() -> [MarginaliaEvent] {
        runtime.pollEvents().map { ev in
            switch ev {
            case .chunkAdvanced(let doc, let sec, let ck):
                return .chunkAdvanced(documentId: doc, section: Int(sec), chunk: Int(ck))
            case .synthesisStarted(let doc, let sec, let ck):
                return .synthesisStarted(documentId: doc, section: Int(sec), chunk: Int(ck))
            case .synthesisReady(let doc, let sec, let ck, let hit):
                return .synthesisReady(documentId: doc, section: Int(sec), chunk: Int(ck), cacheHit: hit)
            case .playbackFinished(let doc, let sec, let ck):
                return .playbackFinished(documentId: doc, section: Int(sec), chunk: Int(ck))
            case .commandRecognized(let raw, let cmd):
                return .commandRecognized(rawText: raw, action: cmd)
            case .sessionRestored(let sid, let doc, let sec, let ck):
                return .sessionRestored(sessionId: sid, documentId: doc, section: Int(sec), chunk: Int(ck))
            case .sessionStopped(let doc):
                return .sessionStopped(documentId: doc)
            case .ingestStarted(let src):
                return .ingestStarted(source: src)
            case .ingestFinished(let src, let docId, let err):
                return .ingestFinished(source: src, documentId: docId, errorMessage: err)
            case .error(let msg):
                return .runtimeError(msg)
            }
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Installable assets — onboarding + Settings downloader
    // ──────────────────────────────────────────────────────────────

    @Published public private(set) var inflightDownloads: [String: InstallUiState] = [:]

    // The install progress poll runs only while something is inflight —
    // no point burning a 2 Hz timer when there's nothing to drain.
    private var installPollTimer: Timer?

    /// Load the asset catalog from the FFI and map to the UI model. Called
    /// at init, and after every asset install/uninstall. Runs on a detached
    /// task because `list_installable_assets` probes the HF cache (one stat
    /// per asset) — not blocking, but not trivial either.
    public func refreshInstallations() async {
        let ffi = self.runtime
        let items: [MarginaliaKit.InstallableAsset] =
            await Task.detached { ffi.listInstallableAssets() }.value
        await MainActor.run {
            self.installations = items.map { rawAsset in
                InstallableAsset(
                    id: rawAsset.id,
                    label: rawAsset.displayName,
                    size: Self.formatBytes(rawAsset.sizeBytes),
                    installed: rawAsset.installed,
                    removable: rawAsset.category == "voice"
                )
            }
            // Clear terminal states for assets that are now installed — the
            // refresh is the canonical "it's done" signal.
            for (id, state) in self.inflightDownloads where state == .installed {
                if self.installations.first(where: { $0.id == id })?.installed == true {
                    self.inflightDownloads.removeValue(forKey: id)
                }
            }
        }
    }

    /// Kick off an install. Returns immediately; watch `inflightDownloads`
    /// for progress. Concurrent installs are allowed (hf-hub is thread-safe;
    /// the Rust side spawns one download thread per call).
    public func installAsset(_ assetId: String) {
        inflightDownloads[assetId] = .queued
        startInstallPolling()
        Task.detached { [runtime, weak self] in
            do {
                try runtime.installAsset(assetId: assetId)
            } catch {
                await MainActor.run {
                    self?.inflightDownloads[assetId] = .failed(
                        "Impossibile avviare il download: \(error.localizedDescription)"
                    )
                }
            }
        }
    }

    /// Remove an installed asset from the HF cache. Synchronous on the FFI
    /// side (one unlink), so we just fire-and-forget and refresh the
    /// catalog afterwards so the row flips back to "non installato".
    public func uninstallAsset(_ id: String) {
        inflightDownloads.removeValue(forKey: id)
        Task.detached { [runtime, weak self] in
            do {
                try runtime.uninstallAsset(assetId: id)
            } catch {
                await MainActor.run {
                    self?.pushMessage("Errore rimozione: \(error.localizedDescription)")
                }
            }
            await self?.refreshInstallations()
        }
    }

    private func startInstallPolling() {
        guard installPollTimer == nil else { return }
        installPollTimer = Timer.scheduledTimer(withTimeInterval: 0.5, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.drainInstallProgress() }
        }
    }

    @MainActor
    private func drainInstallProgress() {
        for frame in runtime.installProgress() {
            switch frame.state {
            case "queued":      inflightDownloads[frame.assetId] = .queued
            case "downloading": inflightDownloads[frame.assetId] = .downloading
            case "installed":
                inflightDownloads[frame.assetId] = .installed
                Task { await self.refreshInstallations() }
            case "error":
                inflightDownloads[frame.assetId] = .failed(frame.errorMessage ?? "sconosciuto")
            default:
                // Forward-compat: unknown state → leave whatever we had.
                break
            }
        }
        // When nothing is moving, stop the timer to let the process idle.
        if inflightDownloads.values.allSatisfy({ $0.isTerminal }) {
            installPollTimer?.invalidate()
            installPollTimer = nil
        }
    }

    private static func formatBytes(_ bytes: UInt64) -> String {
        let mb = Double(bytes) / 1_000_000.0
        if mb >= 100 { return "\(Int(mb.rounded())) MB" }
        if mb >= 1 {
            return String(format: "%.1f MB", mb)
                .replacingOccurrences(of: ".", with: ",")  // Italian locale cosmetic
        }
        let kb = Double(bytes) / 1_000.0
        return "\(Int(kb.rounded())) kB"
    }
}
#endif
