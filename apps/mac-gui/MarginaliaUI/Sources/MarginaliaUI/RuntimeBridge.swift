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
#if canImport(AudioToolbox)
import AudioToolbox
#endif
#if canImport(AVFoundation)
import AVFoundation
#endif
#if canImport(AppKit)
import AppKit
#endif

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
        // Persisted in UserDefaults (key defined in MarginaliaApp) so the
        // flow doesn't restart on next launch. String is duplicated here
        // rather than shared to keep MarginaliaUI buildable without the
        // app target's symbols.
        UserDefaults.standard.set(true, forKey: "marginalia.onboardingComplete")
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
            // Actionable: "Apri log" expands the LogPane so the user
            // sees the full message + prior context (stt:, play:, …)
            // without hunting for it.
            let openLog = ToastAction(label: "apri log") {
                NotificationCenter.default.post(name: .marginaliaToggleLog, object: nil)
            }
            transientToast = ToastMessage(text: m, kind: .error, action: openLog)
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
    /// Two-way bound: slider in Settings and keyboard shortcuts read+write
    /// this. `didSet` forwards to the runtime so the change reaches rodio
    /// immediately (current sink + future sinks).
    @Published public var volume: Double = 1.0 {
        didSet {
            guard oldValue != volume else { return }
            let v = Float(max(0.0, min(2.0, volume)))
            let rt = runtime
            Task.detached { rt.setVolume(level: v) }
        }
    }

    public init(configPath: String) throws {
        self.runtime = try MarginaliaKit.FfiRuntime(configPath: configPath)
        let ff = runtime.currentSpec()
        self.currentSpec = ProviderSpec(
            ttsBackend: ff.ttsBackend, voice: ff.voice,
            sttEngine: ff.sttEngine, language: ff.language
        )
        // Seed volume from runtime — default 1.0 for new instances but
        // read-through in case the runtime persisted it across restarts.
        self.volume = Double(runtime.volume())
        refreshDiscovery()
        // Seed voice commands from the config on disk. Without this the
        // Settings → Comandi vocali table renders empty on first launch
        // (the @Published starts at [] and only `saveVoiceCommands`
        // populated it before).
        self.voiceCommands = runtime.listVoiceCommands().map {
            VoiceCommand(action: $0.action,
                         label: Self.voiceCommandLabel($0.action),
                         triggers: $0.triggers)
        }
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
        // Detached so the SHA-recompute loop inside `listDocuments`
        // (one stat + one sha read per row, off the runtime mutex on
        // the Rust side) doesn't ever block the main thread.
        let ffi = self.runtime
        let docs: [MarginaliaKit.DocumentListItem] =
            await Task.detached { ffi.listDocuments() }.value
        // Counts per doc: chapters + chunks + source_path + needs_reload
        // come free in the FFI list item; notes require a per-doc
        // query (listNotes). N small sqlite reads on refresh — fine
        // for libraries up to a few hundred entries.
        // The "is this doc the active one?" flag is NOT baked here
        // (was: `active: d.id == activeId`). It's derived at render
        // time in the Sidebar from `currentSession?.documentId`, so
        // opening a different doc — which only updates `currentSession`,
        // not `library` — still re-highlights the right row.
        let entries = docs.map { d -> LibraryEntry in
            let noteCount = runtime.listNotes(documentId: d.id).count
            return LibraryEntry(
                id: d.id, title: d.title,
                subtitle: "",  // FFI DocumentListItem doesn't carry author/subtitle today
                progressPct: 0,
                chapterCount: Int(d.chapterCount),
                chunkCount: Int(d.chunkCount),
                notes: noteCount,
                sourcePath: d.sourcePath,
                needsReload: d.needsReload
            )
        }
        await MainActor.run { self.library = entries }
    }

    public func openSourceInEditor(id: String) {
        guard let entry = library.first(where: { $0.id == id }),
              !entry.sourcePath.isEmpty else {
            pushMessage("Errore: percorso del file non disponibile.")
            return
        }
        // NSWorkspace.shared.open returns a Bool — false means the
        // launch failed (no handler registered, file missing, etc.).
        // Surface the failure but don't bubble — the user can pick
        // a different action.
        let url = URL(fileURLWithPath: entry.sourcePath)
        if !NSWorkspace.shared.open(url) {
            pushMessage("Errore: nessuna app registrata per aprire \(entry.sourcePath).")
        }
    }

    public func reloadDocument(id: String) async throws {
        // Re-ingest runs on a detached task so the SHA recompute +
        // chunk re-write don't block the UI. After completion we
        // refresh both the library (clears the reload glyph) and
        // the document view (chunks may have shifted).
        let ffi = self.runtime
        _ = try await Task.detached {
            try ffi.reloadDocument(documentId: id)
        }.value
        await refreshLibrary()
        if currentSession?.documentId == id {
            await refreshDocumentView(id: id)
            await refreshNotes(documentId: id)
        }
    }

    public func openDocument(id: String) async throws {
        try runtime.startSession(documentId: id)
        // Start paused: the runtime begins synthesis/playback on
        // `startSession`, but the user has explicitly asked that opening
        // a document NOT auto-play. An immediate `pauseSession()` halts
        // audio before the first chunk reaches the speakers while
        // keeping the session loaded and positioned at chunk 0, so the
        // play button works normally from there. Ignore failures — the
        // worst case is that playback starts and the user hits pause.
        try? runtime.pauseSession()
        await refreshSessionSnapshot()
        await refreshDocumentView(id: id)
        await refreshNotes(documentId: id)
        // Warm the cache for chunk 1 so when the user finally hits play,
        // chunk 0 starts instantly (already cached by start_session) and
        // chunk 1 is ready by the time auto-advance fires.
        runtime.prefetchNext()
    }

    public func importFile(url: URL) async throws -> String {
        // Log message is emitted from the `.ingestFinished` event handler
        // so it can include chunk count + elapsed time (data computed
        // around the ingest start/finish events, not at this call site).
        let result = try runtime.ingestFile(path: url.path)
        await refreshLibrary()
        return result.documentId
    }

    public func importUrl(_ url: String) async throws -> String {
        let result = try runtime.ingestUrl(url: url)
        await refreshLibrary()
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

    public var ttsCacheDir: String {
        runtime.ttsCacheDir()
    }

    /// Decode the WAV at `path`, downmix to mono f32 at 24kHz, and ship
    /// it to the AEC pipeline as the next render reference. Bridges the
    /// Swift-side `AVAudioPlayer` note playback into the same echo-
    /// cancellation loop that the rodio chunk playback already uses.
    /// Returns false if the file couldn't be decoded (Swift then falls
    /// back to nothing — better an unsuppressed echo than a freeze).
    @discardableResult
    public func aecSetRenderReference(path: String) -> Bool {
        runtime.aecSetRenderReference(path: path)
    }
    public func aecClearRenderReference() {
        runtime.aecClearRenderReference()
    }

    public func synthesizePreview(text: String, voice: String) async throws -> String {
        let lang = currentSpec.language.prefix(2).lowercased()
        return try await Task.detached { [runtime] in
            try runtime.synthesizePreview(text: text, voice: voice, language: String(lang))
        }.value
    }

    public func tick() -> [MarginaliaEvent] {
        // Fire-and-forget auto-advance; its side effects (chunk index bump,
        // synthesis start) arrive back through the event stream. When the
        // runtime actually advanced (return value `true`), spawn a
        // prefetch for the next-next chunk so sequential reading stays
        // fluid without a 1–2 s synthesis gap between chunks.
        if runtime.autoAdvance() {
            runtime.prefetchNext()
        }
        // Also refresh the waveform so the Sidebar footer shows live mic/TTS.
        let snap = runtime.pollWaveform()
        micLevels = snap.micLevels
        ttsLevels = snap.ttsLevels

        // Poll the live partial transcript (Apple STT). When the user
        // is dictating a note, the helper streams "DICT_PARTIAL <text>"
        // lines as the recognizer updates its hypothesis; we copy the
        // latest into `liveNote.body` so the UI renders the running
        // transcript in real time. Empty string means nothing in flight.
        let partial = runtime.dictationPartial()
        if !partial.isEmpty, let live = liveNote, live.live, live.body != partial {
            liveNote = MarginNote(
                id: live.id, chunkId: live.chunkId, when: live.when,
                quote: live.quote, body: partial,
                duration: live.duration, status: live.status, live: true,
                audioReference: live.audioReference
            )
        }

        return pollEvents()
    }

    /// Wall-clock of the most recent `pause()`. Used by `resume()` to
    /// offer a "riparto da inizio chunk" action when the pause was long
    /// enough that the user likely lost context (threshold: 5 min).
    private var lastPausedAt: Date? = nil
    private static let longPauseThreshold: TimeInterval = 300  // 5 min

    /// Wall-clock when the current ingest started. Captured on
    /// `.ingestStarted`, consumed on `.ingestFinished` to emit a log
    /// line with the elapsed time. Nil outside an active ingest.
    private var ingestStartedAt: Date? = nil

    /// Wall-clock when the current TTS synthesis started, captured on
    /// `.synthesisStarted` and consumed on `.synthesisReady` to log
    /// "sintesi Xs" with real timing (cache hits report separately).
    private var synthesisStartedAt: Date? = nil

    public func pause() async throws {
        try runtime.pauseSession()
        await refreshSessionSnapshot()
        await MainActor.run {
            self.lastPausedAt = Date()
            self.showActionFeedback("pause")
        }
    }
    public func resume() async throws {
        try runtime.resumeSession()
        await refreshSessionSnapshot()
        await MainActor.run {
            if let t = self.lastPausedAt,
               Date().timeIntervalSince(t) >= Self.longPauseThreshold
            {
                // Long pause — offer a "riparto da inizio chunk" CTA.
                // The user can accept; if they ignore, resume continues
                // from the cached chunk position.
                let restart = ToastAction(label: "da inizio chunk") { [weak self] in
                    Task { try? await self?.repeatCurrent() }
                }
                self.transientToast = ToastMessage(
                    text: "Ripresa dopo una lunga pausa.",
                    kind: .info,
                    action: restart
                )
            } else {
                self.showActionFeedback("resume")
            }
            self.lastPausedAt = nil
        }
    }
    public func stop() async throws { try runtime.stopSession(); await refreshSessionSnapshot() }
    public func next() async throws {
        try runtime.nextChunk()
        await refreshSessionSnapshot()
        runtime.prefetchNext()
        await MainActor.run { self.showActionFeedback("next") }
    }
    public func back() async throws {
        try runtime.previousChunk()
        await refreshSessionSnapshot()
        runtime.prefetchNext()
        await MainActor.run { self.showActionFeedback("back") }
    }
    public func repeatCurrent() async throws {
        try runtime.repeatChunk()
        await refreshSessionSnapshot()
        runtime.prefetchNext()
        await MainActor.run { self.showActionFeedback("repeat") }
    }
    public func nextChapter() async throws {
        try runtime.nextChapter()
        await refreshSessionSnapshot()
        runtime.prefetchNext()
        await MainActor.run { self.showActionFeedback("next_chapter") }
    }
    public func previousChapter() async throws {
        try runtime.previousChapter()
        await refreshSessionSnapshot()
        runtime.prefetchNext()
        await MainActor.run { self.showActionFeedback("prev_chapter") }
    }
    public func seekToChunk(section: Int, chunk: Int) async throws {
        try runtime.seekToChunk(sectionIndex: UInt32(max(0, section)),
                                 chunkIndex: UInt32(max(0, chunk)))
        await refreshSessionSnapshot()
        runtime.prefetchNext()
    }
    public func seekToChunkPaused(section: Int, chunk: Int) async throws {
        try runtime.seekToChunkPaused(sectionIndex: UInt32(max(0, section)),
                                       chunkIndex: UInt32(max(0, chunk)))
        await refreshSessionSnapshot()
        runtime.prefetchNext()
    }

    /// Kick off a dictation. The runtime thread blocks inside SFSpeech
    /// for up to the configured silence timeout (`[stt.dictation]`
    /// `max_record_seconds`). Progress surfaces through the event stream:
    /// `dictationStarted` flips the live-note card to "recording",
    /// `voiceNoteTranscribed` fills it with the final transcript.
    /// Set when we paused playback to start a dictation. The
    /// `voiceNoteTranscribed` (or cancel) handler reads this and
    /// resumes if true. Self-clearing on every dictation termination.
    private var resumeAfterDictation: Bool = false

    public func startDictation() {
        // If TTS is currently reading, pause it for the duration of
        // the dictation: the model's voice and the user's voice
        // shouldn't overlap, and the AEC has a clearer silence to
        // operate against. Remember the play state so we can resume
        // automatically when dictation finishes (success path).
        if currentSession?.playbackState == .playing {
            resumeAfterDictation = true
            Task { [weak self] in try? await self?.pause() }
        }
        do {
            try runtime.startDictation()
        } catch {
            // Pause already happened above; resume right away since
            // dictation never actually started.
            if resumeAfterDictation {
                resumeAfterDictation = false
                Task { [weak self] in try? await self?.resume() }
            }
            pushMessage("Errore dettatura: \(error.localizedDescription)")
        }
    }

    public func createNote(text: String) async throws {
        guard let s = currentSession else {
            pushMessage("Nessuna sessione attiva.")
            throw NSError(domain: "Marginalia", code: 1,
                          userInfo: [NSLocalizedDescriptionKey: "Nessuna sessione attiva"])
        }
        _ = try runtime.createNote(text: text)
        await refreshNotes(documentId: s.documentId)
        pushMessage("Nota aggiunta · ch.\(s.sectionIndex + 1) chunk \(s.chunkIndex + 1)")
    }

    public func clearLiveNote() { liveNote = nil }

    /// Set when the user cancels the compose-note sheet while dictation
    /// is still in flight. The next `voiceNoteTranscribed` event will
    /// see this flag, delete the just-created note instead of refreshing
    /// notes, then clear the flag. Self-clearing on next emit.
    private var pendingDictationCancelled: Bool = false

    public func cancelPendingDictation() {
        pendingDictationCancelled = true
        liveNote = nil
    }

    public func deleteDocument(id: String) async {
        do {
            try await Task.detached { [runtime] in
                try runtime.deleteDocument(documentId: id)
            }.value
        } catch {
            await MainActor.run {
                self.pushMessage("Errore rimozione: \(error.localizedDescription)")
            }
            return
        }
        // Clear the reader if this was the active doc.
        if currentSession?.documentId == id {
            await refreshSessionSnapshot()
        }
        await refreshLibrary()
    }

    public func deleteNote(id: String) async {
        do {
            try await Task.detached { [runtime] in
                try runtime.deleteNote(noteId: id)
            }.value
        } catch {
            await MainActor.run {
                self.pushMessage("Errore eliminazione nota: \(error.localizedDescription)")
            }
            return
        }
        if let docId = currentSession?.documentId {
            await refreshNotes(documentId: docId)
        }
        await MainActor.run {
            self.pushMessage("Nota eliminata (\(id.prefix(8)))")
        }
    }

    public func updateNote(id: String, text: String) async throws {
        _ = try await Task.detached { [runtime] in
            try runtime.updateNote(noteId: id, newText: text)
        }.value
        if let docId = currentSession?.documentId {
            await refreshNotes(documentId: docId)
        }
        await MainActor.run {
            self.pushMessage("Nota modificata (\(id.prefix(8)))")
        }
    }

    public func exportNotesMarkdown() -> String {
        MockHost.renderNotesMarkdown(
            docTitle: currentSession?.documentTitle ?? "Documento",
            notes: notes
        )
    }

    public func exportBackup(path: String) async throws {
        try await Task.detached { [runtime] in
            try runtime.exportBackup(outPath: path)
        }.value
    }

    public func importBackup(path: String) async throws {
        try await Task.detached { [runtime] in
            try runtime.importBackup(srcPath: path)
        }.value
    }

    public func handle(event: MarginaliaEvent) {
        switch event {
        case .synthesisStarted(_, let sec, let ck):
            synthesizingAnchor = "c\(sec)-\(ck)"
            synthesisStartedAt = Date()
            pushMessage("Sintesi ch.\(sec + 1).\(ck + 1)…")
        case .synthesisReady(_, let sec, let ck, let cacheHit, let elapsedMs):
            synthesizingAnchor = nil
            synthesisStartedAt = nil
            if cacheHit {
                pushMessage("Cache hit ch.\(sec + 1).\(ck + 1)")
            } else {
                // elapsedMs is measured Rust-side around tts.synthesize().
                // Computing it Swift-side from `Date()` between
                // .synthesisStarted and .synthesisReady reads ~0 because
                // the FFI event drainer batches both events into the same
                // poll tick.
                let secs = Double(elapsedMs) / 1000.0
                pushMessage(String(format: "Sintesi ch.%d.%d pronta in %.2fs",
                                   sec + 1, ck + 1, secs))
            }
            Task { await refreshSessionSnapshot() }
        case .chunkAdvanced(_, let sec, let ck):
            pushMessage("▶ ch.\(sec + 1).\(ck + 1)")
            Task { await refreshSessionSnapshot() }
        case .playbackFinished(_, let sec, let ck):
            pushMessage("Fine ch.\(sec + 1).\(ck + 1)")
            Task { await refreshSessionSnapshot() }
        case .sessionRestored, .sessionStopped:
            Task { await refreshSessionSnapshot() }
        case .commandRecognized(let raw, let action):
            // The runtime's `command` field carries the matched trigger
            // word (e.g. "pausa"), not the action name (e.g. "pause") —
            // the TUI's app layer does the trigger→action mapping via
            // `voice_commands.resolve_action`. We do the same here using
            // the locally-cached `voiceCommands` map. Without this the
            // dispatcher case "pausa" misses every action and nothing
            // fires when the user speaks a command.
            //
            // Log every interception (matched OR not) so the user can
            // see in the LogPane what the STT heard, what trigger it
            // matched, and which action got dispatched.
            let resolved = action.flatMap { resolveVoiceAction($0) }
                ?? resolveVoiceAction(raw)
            if let resolved {
                pushMessage("Comando vocale: \"\(raw)\" → \(resolved)")
                dispatchVoiceAction(resolved)
            } else if !raw.isEmpty {
                pushMessage("Comando vocale: \"\(raw)\" (nessun trigger)")
            } else if sttDebug {
                pushMessage("Comando vocale: cattura vuota")
            }
        case .ingestStarted(let source):
            // `source` is either a web URL (scheme + host) or a filesystem
            // path (starts with `/`). Pick the compact form for the
            // overlay label — host for URLs, lastPathComponent for files.
            let label: String = {
                if let url = URL(string: source), url.scheme != nil,
                   !source.hasPrefix("/") {
                    return url.host ?? url.lastPathComponent
                }
                // File path: strip to just the filename.
                return (source as NSString).lastPathComponent
            }()
            ingestingSource = label
            ingestStartedAt = Date()
        case .ingestFinished(let source, let docId, let err):
            ingestingSource = nil
            let elapsed = ingestStartedAt.map { -$0.timeIntervalSinceNow } ?? 0
            ingestStartedAt = nil
            if let err {
                pushMessage("Errore import: \(err)")
                // Actionable: "Riprova" re-runs the same import path.
                // Detect URL-vs-file the same way `ingestStarted` does:
                // filesystem paths start with `/` and have no scheme.
                let retry = ToastAction(label: "riprova") { [weak self] in
                    guard let self = self else { return }
                    let isWebUrl = !source.hasPrefix("/")
                        && URL(string: source)?.scheme != nil
                    if isWebUrl {
                        Task { _ = try? await self.importUrl(source) }
                    } else {
                        let fileUrl = URL(fileURLWithPath: source)
                        Task { _ = try? await self.importFile(url: fileUrl) }
                    }
                }
                transientToast = ToastMessage(
                    text: "Import fallito: \(err)",
                    kind: .error,
                    action: retry
                )
            } else {
                // Probe the document view for a chunk count so the log
                // line tells the user how much work the ingest produced.
                // `documentView` reads from sqlite — cheap, synchronous.
                if let docId, let view = runtime.documentView(documentId: docId) {
                    let chunks = view.sections.reduce(0) { $0 + $1.chunks.count }
                    pushMessage(String(
                        format: "Importato: %@ — %d chunk in %.2fs",
                        view.title, chunks, elapsed
                    ))
                } else {
                    pushMessage(String(format: "Import completato in %.2fs", elapsed))
                }
                Task { await refreshLibrary() }
            }
        case .dictationStarted:
            // Seed the live-note card in "recording" state so the user
            // sees immediate feedback the helper has switched mode.
            liveNote = MarginNote(
                id: "live", chunkId: currentSession?.anchor ?? "",
                when: "ora", quote: "", body: "",
                duration: "0:00", status: "", live: true
            )
        case .voiceNoteTranscribed(let text, let dur, let noteId, let err):
            // Auto-resume the TTS reading if we paused it on dictation
            // start (whatever the outcome — success, error, or cancel).
            // The flag is consumed here so a stray subsequent event
            // can't trigger a phantom resume.
            let shouldResumePlayback = resumeAfterDictation
            resumeAfterDictation = false
            // Consume the cancel flag UNCONDITIONALLY so it can never
            // bleed across to a later, unrelated dictation. Without
            // this, a previous dictation that ended in error (silence
            // timeout / empty transcript) left the flag set, and the
            // *next* successful dictation would silently delete the
            // freshly-created note — manifesting as "every now and
            // then a previous note disappears when I record a new one".
            let wasCancelled = pendingDictationCancelled
            pendingDictationCancelled = false
            defer {
                if shouldResumePlayback {
                    Task { [weak self] in try? await self?.resume() }
                }
            }
            if let err {
                liveNote = nil
                // Suppress the noisy toast when the user already
                // cancelled (sheet cancel OR sheet save with typed
                // text — both call `cancelPendingDictation`). The
                // dictation timeout / empty-transcript event still
                // arrives a few seconds later, but the user neither
                // wants to see "errore dettatura" nor needs the mic-
                // unplugged hint in that flow.
                if wasCancelled { break }
                pushMessage("Errore dettatura: \(err)")
                // B14 — discriminate "no audio captured" from "you
                // didn't speak". If the OS reports no audio input
                // device, assume the mic is unplugged / disabled.
                if Self.noMicInputAvailable() {
                    let openPrefs = ToastAction(label: "apri preferenze") {
                        #if canImport(AppKit)
                        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.sound?input") {
                            NSWorkspace.shared.open(url)
                        }
                        #endif
                    }
                    transientToast = ToastMessage(
                        text: "Microfono non disponibile. Controlla che sia collegato e selezionato.",
                        kind: .error,
                        action: openPrefs
                    )
                } else {
                    transientToast = ToastMessage(
                        text: "Dettatura fallita: \(err)", kind: .error
                    )
                }
            } else {
                // The user pressed Cancel on the compose sheet while
                // the dictation thread was still running. The runtime
                // has just persisted a note from whatever audio was
                // captured before the cancel; delete it back out.
                // (The flag was already consumed at the top of the
                // case so a later dictation can't inherit it.)
                if wasCancelled {
                    liveNote = nil
                    if let id = noteId {
                        Task { await self.deleteNote(id: id) }
                    }
                    pushMessage("Dettatura annullata, nota scartata")
                    break
                }
                // Promote the live card to a saved note and refresh the
                // canonical notes list so it shows up in the margin panel.
                liveNote = MarginNote(
                    id: noteId ?? "live",
                    chunkId: currentSession?.anchor ?? "",
                    when: "ora", quote: "", body: text,
                    duration: String(format: "%d:%02d",
                                     Int(dur) / 60, Int(dur) % 60),
                    status: "salvata", live: true
                )
                if let docId = currentSession?.documentId {
                    // Refresh notes, then clear the live card. Doing the
                    // clear inside the same Task — AFTER refreshNotes
                    // populates host.notes with the persisted note —
                    // means the margin panel never shows the same note
                    // twice. The de-dup guard in `notes` covers any
                    // remaining edge case.
                    Task { [weak self] in
                        await self?.refreshNotes(documentId: docId)
                        await MainActor.run { self?.liveNote = nil }
                    }
                } else {
                    Task { [weak self] in
                        try? await Task.sleep(nanoseconds: 1_500_000_000)
                        await MainActor.run { self?.liveNote = nil }
                    }
                }
            }
        case .voiceMismatch(_, let detected, let current):
            // The runtime detected the document in language X but the
            // current voice is language Y. Pick the best available voice
            // in X (if any is installed) and offer a one-click swap.
            let currentPrefix = String(current.prefix(2)).lowercased()
            guard currentPrefix != detected else { break }
            let candidates = voices.filter {
                $0.lang.lowercased().hasPrefix(detected)
            }
            let bestVoice = candidates.first?.id
            let langName = Self.languageLabel(detected)
            let action: ToastAction?
            if let bestVoice = bestVoice {
                let bcp47 = candidates.first?.lang ?? detected
                action = ToastAction(label: "passa a \(langName)") { [weak self] in
                    guard let self = self else { return }
                    let newSpec = ProviderSpec(
                        ttsBackend: self.currentSpec.ttsBackend,
                        voice: bestVoice,
                        sttEngine: self.currentSpec.sttEngine,
                        language: bcp47
                    )
                    Task { try? await self.apply(spec: newSpec) }
                }
            } else {
                // No installed voice for the detected language — the
                // actionable suggestion is "install", but the catalog
                // lookup lives in Settings, so just point the user there.
                action = ToastAction(label: "installa voci") { [weak self] in
                    self?.pushMessage("Apri Impostazioni → Installazioni per aggiungere voci \(langName).")
                }
            }
            transientToast = ToastMessage(
                text: "Questo documento sembra in \(langName).",
                kind: .info,
                action: action
            )
        case .runtimeError(let msg):
            pushMessage("Errore: \(msg)")
        }
    }

    /// True when the OS reports zero audio input devices — used to
    /// discriminate "user didn't speak" (normal empty dictation) from
    /// "mic unplugged" in the `voiceNoteTranscribed` error path.
    ///
    /// Conservative: if AVFoundation is unavailable (non-macOS build)
    /// or the discovery session fails, returns false so we don't
    /// spuriously claim the mic is gone.
    private static func noMicInputAvailable() -> Bool {
        #if canImport(AVFoundation)
        // Any "builtInMicrophone" or "externalUnknown" device counts.
        // The system mic (even if muted in sound prefs) shows up here —
        // genuine unplug + no internal mic is the only way to get an
        // empty list on a Mac.
        let session = AVCaptureDevice.DiscoverySession(
            deviceTypes: [.microphone, .external],
            mediaType: .audio,
            position: .unspecified
        )
        return session.devices.isEmpty
        #else
        return false
        #endif
    }

    /// Display label for a voice-command action id. Mirrors the labels
    /// the MockHost's `voiceCommands` default uses so the Settings row
    /// order + strings match across mock and live hosts.
    private static func voiceCommandLabel(_ action: String) -> String {
        switch action {
        case "pause":        return "Pausa"
        case "resume":       return "Riprendi"
        case "next":         return "Prossimo chunk"
        case "back":         return "Chunk precedente"
        case "repeat":       return "Ripeti"
        case "stop":         return "Ferma"
        case "next_chapter": return "Prossimo capitolo"
        case "prev_chapter": return "Capitolo precedente"
        case "bookmark":     return "Salva posizione"
        case "note":         return "Nuova nota"
        case "where":        return "Dove sono"
        default:             return action
        }
    }

    /// Italian-facing language name for the short BCP-47 prefix
    /// surfaced by the runtime's whatlang detection. Matches the
    /// languages Kokoro ships voices for.
    private static func languageLabel(_ prefix: String) -> String {
        switch prefix.lowercased() {
        case "en": return "inglese"
        case "it": return "italiano"
        case "fr": return "francese"
        case "de": return "tedesco"
        case "es": return "spagnolo"
        case "pt": return "portoghese"
        case "ja": return "giapponese"
        case "zh": return "cinese"
        case "hi": return "hindi"
        default:   return prefix
        }
    }

    /// Map a recognized voice command to the right host method. Mirrors the
    /// TUI's `resolve_action` dispatch in `apps/tui-rs/src/app.rs:717`.
    ///
    /// The confirmation toast is emitted by the playback methods
    /// themselves (`pause()`, `next()`, etc. → `showActionFeedback`) so
    /// keyboard shortcuts and voice commands get identical UI. Here we
    /// only play the "heard you" chime — the audible half of the
    /// confirmation.
    /// Map a raw STT phrase or single trigger word back to an action
    /// name. Mirror of `VoiceCommandsSection::resolve_action` on the
    /// Rust side: longer multi-word triggers ("prossimo capitolo") are
    /// checked before shorter ones ("prossimo") so a command like
    /// "next_chapter" doesn't get swallowed by "next". Returns nil if
    /// no trigger is contained in the raw text.
    private func resolveVoiceAction(_ raw: String) -> String? {
        let lower = raw.lowercased()
        let priority = [
            "next_chapter", "prev_chapter",
            "bookmark", "note", "where",
            "pause", "resume", "next", "back", "stop", "repeat",
        ]
        for action in priority {
            guard let cmd = voiceCommands.first(where: { $0.action == action }) else {
                continue
            }
            if cmd.triggers.contains(where: { lower.contains($0.lowercased()) }) {
                return action
            }
        }
        return nil
    }

    private func dispatchVoiceAction(_ action: String) {
        pushMessage("→ \(action)")
        playCommandChime()
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
                // Dictation via voice: pause TTS playback first so the
                // user's voice doesn't have to compete with the model's
                // (and the AEC has a stable silence to operate against),
                // then fire the FFI. `DictationStarted` event seeds the
                // live-note card, `VoiceNoteTranscribed` fills it with
                // the transcript. Mirrors the "+" button flow in
                // `ReadingView.openComposeNote`.
                if self.currentSession?.playbackState == .playing {
                    try? await self.pause()
                }
                await MainActor.run { self.startDictation() }
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
                    duration: "", status: "", live: false,
                    audioReference: n.audioReference
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
            case .synthesisReady(let doc, let sec, let ck, let hit, let elapsedMs):
                return .synthesisReady(documentId: doc, section: Int(sec), chunk: Int(ck), cacheHit: hit, elapsedMs: elapsedMs)
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
            case .dictationStarted:
                return .dictationStarted
            case .voiceNoteTranscribed(let text, let dur, let nid, let err):
                return .voiceNoteTranscribed(text: text, durationSecs: dur,
                                              noteId: nid, errorMessage: err)
            case .voiceMismatch(let doc, let detected, let current):
                return .voiceMismatch(documentId: doc,
                                       detectedLanguage: detected,
                                       currentLanguage: current)
            case .error(let msg):
                return .runtimeError(msg)
            }
        }
    }

    // ──────────────────────────────────────────────────────────────
    // Installable assets — onboarding + Settings downloader
    // ──────────────────────────────────────────────────────────────

    @Published public private(set) var inflightDownloads: [String: InstallUiState] = [:]

    /// Non-nil while a remote-catalog refresh is in flight. UI watches this
    /// to disable the "Aggiorna lista voci" button and show a small spinner.
    @Published public private(set) var catalogRefreshInflight: Bool = false
    /// Last-attempt outcome (success message or error). Cleared by the next
    /// invocation. Used to surface "scaricate N voci" / "rete non disponibile"
    /// in Settings.
    @Published public var catalogRefreshStatus: String? = nil

    // The install progress poll runs only while something is inflight —
    // no point burning a 2 Hz timer when there's nothing to drain.
    private var installPollTimer: Timer?

    /// Refresh the voice catalog from huggingface.co. **Network call** —
    /// only fired on explicit user action: onboarding `installModels` step
    /// (auto on appear) or the "Aggiorna lista voci" button in Settings.
    /// On success the catalog cache file is rewritten and `installations`
    /// is reloaded so the new voices show up immediately. On failure the
    /// previous list stays intact and `catalogRefreshStatus` carries the
    /// error message.
    public func refreshRemoteVoiceCatalog() async {
        await MainActor.run {
            self.catalogRefreshInflight = true
            self.catalogRefreshStatus = nil
        }
        let ffi = self.runtime
        let result: Result<UInt32, Error> = await Task.detached {
            do {
                let count = try ffi.fetchRemoteVoiceCatalog()
                return .success(count)
            } catch {
                return .failure(error)
            }
        }.value
        switch result {
        case .success(let count):
            await refreshInstallations()
            await MainActor.run {
                self.catalogRefreshInflight = false
                self.catalogRefreshStatus = "Lista aggiornata: \(count) voci."
            }
        case .failure(let err):
            await MainActor.run {
                self.catalogRefreshInflight = false
                self.catalogRefreshStatus = "Aggiornamento fallito: \(err.localizedDescription)"
            }
        }
    }

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
                    // All installed assets are user-removable —
                    // including individual voices. Voices are small
                    // (~500 KB) but a user may want to drop the ones
                    // they never use; the picker state recovers
                    // automatically (the row flips back to "scarica").
                    removable: rawAsset.installed,
                    category: rawAsset.category,
                    language: rawAsset.language
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
            case "downloading":
                // bytes_total == 0 means hf-hub hasn't called init() yet
                // (first downloading frame emitted before the network
                // handshake). Render as indeterminate in that window.
                let fraction: Double? = frame.bytesTotal > 0
                    ? min(1.0, Double(frame.bytesDone) / Double(frame.bytesTotal))
                    : nil
                inflightDownloads[frame.assetId] = .downloading(fraction: fraction)
            case "installed":
                inflightDownloads[frame.assetId] = .installed
                Task { await self.refreshInstallations() }
                // Voices affect the Settings picker + TtsBackends list.
                // Re-run discovery so the Voce panel surfaces the new
                // entry without the user having to close and reopen
                // Settings (a frequently-reported UX rough-edge).
                if frame.assetId.hasPrefix("voice:") || frame.assetId == "mlx-core" {
                    refreshDiscovery()
                }
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

    /// Visual confirmation for playback actions, fired regardless of
    /// source (voice command, keyboard shortcut, menu click). Bookmarks
    /// and notes get their own dedicated feedback paths; this covers the
    /// fast path of pause/next/back/etc.
    @MainActor
    func showActionFeedback(_ action: String) {
        transientToast = ToastMessage(text: voiceFeedbackLabel(for: action), kind: .info)
    }

    /// Short human-readable label shown in the confirmation toast when a
    /// voice command is recognized. Matches the Italian STT vocabulary —
    /// unknown actions fall back to the raw identifier.
    private func voiceFeedbackLabel(for action: String) -> String {
        switch action {
        case "pause":        return "pausa"
        case "resume":       return "riprendi"
        case "next":         return "prossimo"
        case "back":         return "indietro"
        case "repeat":       return "ripeti"
        case "stop":         return "stop"
        case "next_chapter": return "capitolo +"
        case "prev_chapter": return "capitolo −"
        case "bookmark":     return "segnalibro"
        case "note":         return "nota"
        case "where":        return "posizione"
        default:             return action
        }
    }

    /// Play the system Tink as command-match confirmation. No-op when
    /// AppKit isn't available (non-macOS build). Runs on the caller thread
    /// — AudioServices is lightweight + async internally.
    private func playCommandChime() {
        #if canImport(AudioToolbox)
        // 1057 = Tink (the "positive" chime). Safe to call repeatedly.
        AudioServicesPlaySystemSound(SystemSoundID(1057))
        #endif
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
