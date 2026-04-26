import Foundation

// MARK: — Discovery types (mirror of marginalia-ffi records)

public enum Gender: String, Codable, Sendable {
    case female, male, unknown
}

public struct VoiceInfo: Identifiable, Hashable, Codable, Sendable {
    public let id: String          // "if_sara"
    public let display: String     // "Sara"
    public let lang: String        // "it-IT"
    public let gender: Gender
    public let backend: String     // "mlx"
    public let installed: Bool     // derived: file present under voices/

    public init(id: String, display: String, lang: String,
                gender: Gender, backend: String, installed: Bool) {
        self.id = id
        self.display = display
        self.lang = lang
        self.gender = gender
        self.backend = backend
        self.installed = installed
    }
}

public struct LangInfo: Identifiable, Hashable, Codable, Sendable {
    public var id: String { bcp47 }
    public let bcp47: String       // "it-IT"
    public let display: String     // "italiano"
    public let voiceCount: Int     // number of installed voices for this lang

    public init(bcp47: String, display: String, voiceCount: Int) {
        self.bcp47 = bcp47
        self.display = display
        self.voiceCount = voiceCount
    }
}

public struct TtsBackendInfo: Identifiable, Hashable, Codable, Sendable {
    public var id: String { backendId }
    public let backendId: String   // "mlx"
    public let name: String        // "Kokoro MLX"
    public let sub: String         // "Apple Silicon · Metal"
    public let available: Bool
    public let reason: String?
}

public struct SttEngineInfo: Identifiable, Hashable, Codable, Sendable {
    public var id: String { engineId }
    public let engineId: String    // "apple"
    public let name: String        // "Apple Speech"
    public let available: Bool
    public let reason: String?
    public let note: String        // user-facing hint
}

// MARK: — ProviderSpec / ApplyReport (mirror of marginalia-ffi)

public struct ProviderSpec: Equatable, Codable, Sendable {
    public var ttsBackend: String
    public var voice: String
    public var sttEngine: String
    public var language: String

    public init(ttsBackend: String, voice: String, sttEngine: String, language: String) {
        self.ttsBackend = ttsBackend
        self.voice = voice
        self.sttEngine = sttEngine
        self.language = language
    }
}

public struct ApplyReport: Equatable, Codable, Sendable {
    public var ttsSwapped: Bool
    public var sttSwapped: Bool
    public var languageChanged: Bool
    public var elapsedMs: UInt64
    public init(ttsSwapped: Bool = false, sttSwapped: Bool = false,
                languageChanged: Bool = false, elapsedMs: UInt64 = 0) {
        self.ttsSwapped = ttsSwapped
        self.sttSwapped = sttSwapped
        self.languageChanged = languageChanged
        self.elapsedMs = elapsedMs
    }
}

// MARK: — Voice commands

public struct VoiceCommand: Identifiable, Hashable, Codable, Sendable {
    public var id: String { action }
    public let action: String      // "pause"
    public let label: String       // "Metti in pausa"
    public var triggers: [String]  // ["pausa", "ferma"]

    public init(action: String, label: String, triggers: [String]) {
        self.action = action
        self.label = label
        self.triggers = triggers
    }
}

// MARK: — Installable assets

public struct InstallableAsset: Identifiable, Hashable, Codable, Sendable {
    public let id: String
    public let label: String
    public let size: String        // "74 MB", "465 MB"
    public var installed: Bool
    public let removable: Bool
    /// Mirrors the FFI `category` field — "tts_core" | "voice" | "stt"
    /// | "importer". Used by the Installations panel to group voices
    /// under a disclosure by language rather than listing them flat.
    public let category: String
    /// BCP-47 code for voices ("it-IT", "en-US"); nil for core models.
    public let language: String?

    public init(id: String, label: String, size: String,
                installed: Bool, removable: Bool,
                category: String = "tts_core", language: String? = nil) {
        self.id = id; self.label = label; self.size = size
        self.installed = installed; self.removable = removable
        self.category = category; self.language = language
    }
}

/// UI-side install state surfaced by hosts (live and mock). Mirrors the
/// FFI's `InstallProgress` state string but typed for safety. Missing keys
/// in `MarginaliaHost.inflightDownloads` mean the asset is idle (not
/// installed, not inflight).
///
/// `downloading(fraction:)` carries 0.0–1.0 when the backend supplied a
/// total; the preview/mock and early "downloading" frames (before hf-hub
/// has negotiated Content-Length) use `nil` which the UI renders as an
/// indeterminate spinner.
public enum InstallUiState: Equatable, Hashable, Sendable {
    case queued
    case downloading(fraction: Double?)
    case installed
    case failed(String)

    public var isTerminal: Bool {
        switch self {
        case .installed, .failed: return true
        case .queued, .downloading: return false
        }
    }

    /// Convenience for `if host.inflightDownloads[id]?.isInflight == true`
    /// without pattern-matching overhead. Unknown states (forward compat)
    /// count as not inflight.
    public var isInflight: Bool {
        if case .queued = self { return true }
        if case .downloading = self { return true }
        return false
    }
}

// MARK: — Reading view models (mock for now; real app passes domain types)

public struct ReadingChunk: Identifiable, Hashable, Sendable {
    public let id: String          // anchor: "s2-c5" (section 2, chunk 5)
    public let index: Int          // chunk index within its section (runtime-side)
    public let text: String
    public init(id: String, index: Int = 0, text: String) {
        self.id = id; self.index = index; self.text = text
    }
}

public struct MarginNote: Identifiable, Hashable, Sendable {
    public let id: String
    public let chunkId: String
    public let when: String        // "10 min fa"
    public let quote: String
    public let body: String
    public let duration: String    // "0:21"
    public let status: String      // "" | "applicato" | "rielaborato"
    public let live: Bool
    /// Absolute path of the persisted WAV recorded during dictation.
    /// `nil` for typed notes and bookmarks — the playback path falls
    /// back to TTS synthesis of the transcript in that case.
    public let audioReference: String?
    public init(id: String, chunkId: String, when: String, quote: String,
                body: String, duration: String, status: String, live: Bool = false,
                audioReference: String? = nil) {
        self.id = id; self.chunkId = chunkId; self.when = when
        self.quote = quote; self.body = body; self.duration = duration
        self.status = status; self.live = live
        self.audioReference = audioReference
    }
}

public struct LibraryEntry: Identifiable, Hashable, Sendable {
    public let id: String          // document id from the runtime (or UUID in the mock)
    public let title: String
    public let subtitle: String    // "Thomas Mann" | "bozza"
    public let progressPct: Int
    public let chapterCount: Int
    public let chunkCount: Int
    public let notes: Int
    public let active: Bool
    /// Filesystem path of the source file at ingestion time. Empty
    /// string when the host doesn't know it (mock fixtures, legacy
    /// rows). Drives the "Apri file in editor…" context-menu entry.
    public let sourcePath: String
    /// True iff the file currently on disk has a different SHA than the
    /// one stored at ingestion time. Drives the reload glyph in the
    /// sidebar row and enables the "Ricarica da disco" context-menu
    /// entry. Computed by the runtime; the mock keeps it false.
    public let needsReload: Bool

    public init(id: String, title: String, subtitle: String,
                progressPct: Int, chapterCount: Int = 0, chunkCount: Int = 0,
                notes: Int, active: Bool,
                sourcePath: String = "", needsReload: Bool = false) {
        self.id = id; self.title = title; self.subtitle = subtitle
        self.progressPct = progressPct
        self.chapterCount = chapterCount; self.chunkCount = chunkCount
        self.notes = notes; self.active = active
        self.sourcePath = sourcePath; self.needsReload = needsReload
    }
}

// MARK: — Reader / playback state

public enum PlaybackState: String, Sendable, Codable {
    case idle, playing, paused, finished, unknown
}

public struct SessionState: Hashable, Sendable {
    public let sessionId: String
    public let documentId: String
    public let documentTitle: String
    public let sectionIndex: Int
    public let sectionCount: Int
    public let sectionTitle: String
    public let chunkIndex: Int
    public let chunkText: String
    public let anchor: String
    public let playbackState: PlaybackState
    public let notesCount: Int
    public let voice: String?

    public init(sessionId: String, documentId: String, documentTitle: String,
                sectionIndex: Int, sectionCount: Int, sectionTitle: String,
                chunkIndex: Int, chunkText: String, anchor: String,
                playbackState: PlaybackState, notesCount: Int, voice: String?) {
        self.sessionId = sessionId; self.documentId = documentId
        self.documentTitle = documentTitle
        self.sectionIndex = sectionIndex; self.sectionCount = sectionCount
        self.sectionTitle = sectionTitle; self.chunkIndex = chunkIndex
        self.chunkText = chunkText; self.anchor = anchor
        self.playbackState = playbackState; self.notesCount = notesCount
        self.voice = voice
    }
}

public struct SectionDoc: Hashable, Sendable {
    public let index: Int
    public let title: String
    public let chunks: [ReadingChunk]
    public init(index: Int, title: String, chunks: [ReadingChunk]) {
        self.index = index; self.title = title; self.chunks = chunks
    }
}

public struct DocumentDoc: Hashable, Sendable {
    public let documentId: String
    public let title: String
    public let sections: [SectionDoc]
    public init(documentId: String, title: String, sections: [SectionDoc]) {
        self.documentId = documentId; self.title = title; self.sections = sections
    }
}

// Live / saved notes with runtime provenance.
public extension MarginNote {
    init(saved id: String, documentId: String, sectionIndex: Int, chunkIndex: Int,
         anchor: String, text: String, createdAtIso: String) {
        self.init(
            id: id, chunkId: anchor,
            when: createdAtIso.prefix(10).description,
            quote: "", body: text,
            duration: "", status: "", live: false
        )
    }
}

public struct ToastMessage: Identifiable, Sendable {
    public let id = UUID()
    public let text: String
    public let kind: Kind
    /// Optional CTA — label + handler. When non-nil, ToastOverlay renders
    /// a button inside the toast. Tapping it runs `action()` and (by
    /// convention) dismisses the toast. Use for "Riprova", "Apri
    /// Impostazioni", "Vedi log", etc. — anything better than the user
    /// having to navigate elsewhere to recover.
    public let action: ToastAction?
    public enum Kind: Sendable { case info, warning, error }

    public init(text: String, kind: Kind = .info, action: ToastAction? = nil) {
        self.text = text
        self.kind = kind
        self.action = action
    }
}

/// Action attached to a toast. `Sendable` closures so the struct stays
/// thread-safe — ToastOverlay invokes the handler on the main actor.
public struct ToastAction: Sendable {
    public let label: String
    public let handler: @Sendable @MainActor () -> Void
    public init(label: String, handler: @Sendable @MainActor @escaping () -> Void) {
        self.label = label
        self.handler = handler
    }
}

public enum MarginaliaEvent: Sendable {
    case chunkAdvanced(documentId: String, section: Int, chunk: Int)
    /// Synthesis kicked off — audio not ready yet. The gap between this
    /// and `synthesisReady` is what the "sintetizzando…" indicator covers.
    case synthesisStarted(documentId: String, section: Int, chunk: Int)
    case synthesisReady(documentId: String, section: Int, chunk: Int, cacheHit: Bool, elapsedMs: UInt64)
    case playbackFinished(documentId: String, section: Int, chunk: Int)
    case commandRecognized(rawText: String, action: String?)
    case sessionRestored(sessionId: String, documentId: String, section: Int, chunk: Int)
    case sessionStopped(documentId: String)
    case ingestStarted(source: String)
    case ingestFinished(source: String, documentId: String?, errorMessage: String?)
    case dictationStarted
    case voiceNoteTranscribed(text: String, durationSecs: Double, noteId: String?, errorMessage: String?)
    /// The document's detected language doesn't match the currently
    /// selected voice's language — UI should prompt for a switch.
    case voiceMismatch(documentId: String, detectedLanguage: String, currentLanguage: String)
    case runtimeError(String)
}

// MARK: — Host protocol

/// What the UI needs from the host (either the marginalia-ffi-backed
/// adapter or the SwiftUI preview mock). Abstracting this lets the
/// library compile without a Rust dependency.
@MainActor
public protocol MarginaliaHost: AnyObject, ObservableObject {
    var currentSpec: ProviderSpec { get }
    var languages: [LangInfo] { get }
    var voices: [VoiceInfo] { get }
    var sttEngines: [SttEngineInfo] { get }
    var ttsBackends: [TtsBackendInfo] { get }
    var voiceCommands: [VoiceCommand] { get }
    var installations: [InstallableAsset] { get }
    /// Per-asset transient state for downloads in progress. Missing keys
    /// mean "not currently downloading". Live host updates this from the
    /// `install_progress` FFI poll; mock host keeps it empty.
    var inflightDownloads: [String: InstallUiState] { get }

    // Reader-side state.
    var library: [LibraryEntry] { get }
    var currentSession: SessionState? { get }
    var currentDocument: DocumentDoc? { get }
    var notes: [MarginNote] { get }
    var liveNote: MarginNote? { get }

    /// Non-nil while the TTS backend is synthesizing a chunk. The Toolbar
    /// reads this to render a spinner alongside the document title so the
    /// ~1 s synthesis latency doesn't look like a frozen app. Mock hosts
    /// can keep this nil; the UX difference between "ready" and "synth"
    /// only matters with a real backend.
    var synthesizingAnchor: String? { get }

    /// Non-nil while an import job (drag-drop, ⌘O, ⌘U) is being chunked
    /// and saved. The window overlays a blocking "sto leggendo …" card
    /// so large PDFs don't feel like a freeze. Value is a human-readable
    /// source label ("libro.pdf", "https://…").
    var ingestingSource: String? { get }

    /// Persisted reading-side settings that don't live in the ProviderSpec.
    var chunkTargetChars: Int { get }
    var sttDebug: Bool { get }

    /// True iff this host has yet to complete first-run setup (marginalia.toml
    /// and core models present). The app routes to WelcomeView/InstallModelsView
    /// when this is true.
    var needsOnboarding: Bool { get }
    /// Flag the host as onboarded (persists on disk). Called from the
    /// onboarding flow's "procedi" button.
    func markOnboardingComplete()

    /// Blocking apply; the real host runs it on a background task and returns
    /// only when the sidecar thread replies. The mock resolves after a delay.
    func apply(spec: ProviderSpec) async throws -> ApplyReport

    // Library / sessions / playback. All throw on failure; the UI shows an
    // inline error toast. Voice commands firing the same action go through
    // the same methods.
    func refreshLibrary() async
    func openDocument(id: String) async throws
    func importFile(url: URL) async throws -> String    // returns new document id
    func importUrl(_ url: String) async throws -> String
    func pause() async throws
    func resume() async throws
    func stop() async throws
    func next() async throws
    func back() async throws
    func repeatCurrent() async throws
    func nextChapter() async throws
    func previousChapter() async throws
    /// Jump to a specific `(section, chunk)` position in the active document.
    /// Wired to chunk-click in the reading column.
    func seekToChunk(section: Int, chunk: Int) async throws
    /// Same as `seekToChunk` but the runtime pauses playback immediately
    /// after re-loading the new position, so single-click on a note
    /// repositions silently. Without this the runtime auto-plays on
    /// every seek (its `replay_session_at_position` always calls
    /// `playback_engine.start` which kicks rodio into Playing).
    func seekToChunkPaused(section: Int, chunk: Int) async throws

    /// Fire-and-forget: start a voice-note dictation. The live note card
    /// lights up via `dictationStarted`, the final transcript arrives via
    /// `voiceNoteTranscribed`.
    func startDictation()

    /// Create a note with the supplied typed text, auto-attached to the
    /// current reading position on the runtime side. Throws on failure
    /// (no active session, storage error).
    func createNote(text: String) async throws

    /// Dismiss the pending live-note state. Used by the compose-note
    /// sheet on cancel (so the in-flight dictation card doesn't linger
    /// after we've deleted its backing persisted note).
    func clearLiveNote()

    /// Mark the next-to-arrive `voiceNoteTranscribed` as cancelled —
    /// the runtime creates the note synchronously after dictation
    /// stops, so a Cancel pressed BEFORE the silence-final still ends
    /// up persisting a note. Setting this flag tells the host to
    /// delete that note as soon as it's created. Self-clearing.
    func cancelPendingDictation()

    /// Remove a document from the library. Cascades to chunks, notes,
    /// sessions in storage. If the doc is the active one, the host stops
    /// the session first so the reader doesn't render a ghost chunk.
    func deleteDocument(id: String) async

    /// Open the source file in the OS default editor for that file
    /// type (`NSWorkspace.shared.open`). Sync, side-effect only — the
    /// OS handles failure surfacing (no-handler dialog). Mock host can
    /// log and skip.
    func openSourceInEditor(id: String)
    /// Re-ingest a document's source file from disk. Used after the
    /// user edits the file externally (badge `needsReload` is set).
    /// Path-stable id keeps notes/sessions attached. Refresh of
    /// `library` happens inside the host after the call resolves.
    func reloadDocument(id: String) async throws

    /// Delete a note by id. Idempotent — unknown ids no-op. After deletion
    /// the host should refresh `notes`.
    func deleteNote(id: String) async
    /// Update the transcript of an existing note. Host refreshes `notes`
    /// on success so the margin panel picks up the edit.
    func updateNote(id: String, text: String) async throws
    /// Render all notes for the active document as Markdown. Used by the
    /// menu's `Esporta note…` action. Empty string when no session.
    func exportNotesMarkdown() -> String

    /// Write a backup zip (config + sqlite + voices manifest) to `path`.
    /// Throws on I/O errors.
    func exportBackup(path: String) async throws
    /// Restore a backup zip from `path`. The caller is expected to
    /// terminate the app afterwards — the runtime doesn't hot-reload
    /// from swapped-out files.
    func importBackup(path: String) async throws

    /// Linear playback volume, 0.0–1.0. Rodio accepts >1.0 as amplifi-
    /// cation but the slider caps at 1.0 to avoid distortion by default.
    var volume: Double { get set }

    /// Push the WAV at `path` into the AEC pipeline as the render
    /// reference for audio the GUI is about to play *outside* of the
    /// rodio host engine — currently Swift `AVAudioPlayer` for note
    /// playback. AEC3 will then subtract that signal from the mic
    /// capture so a note whose body contains "nota" (or any other
    /// voice trigger) doesn't auto-fire commands when the speaker
    /// echoes back into the microphone. Returns false on decode
    /// failure or when AEC isn't compiled in (e.g. non-Apple STT) so
    /// the caller can fall back to suppressing commands manually.
    @discardableResult
    func aecSetRenderReference(path: String) -> Bool
    /// Clear the AEC render reference (call when the player stops or
    /// drains). Idempotent — no-op when no reference is loaded.
    func aecClearRenderReference()
    /// Save a "[BOOKMARK] …" note at the current position (voice: "segna").
    func bookmark() async throws
    /// Human-readable position string (voice: "dove sono"). Callers
    /// either speak it aloud or append to the log pane.
    func announcePosition() -> String?

    /// Transient log / status messages (voice command echoes, errors,
    /// informational toasts). Bounded to ~64 entries by convention.
    var messages: [String] { get }

    /// Append a line to `messages`. Callers are any view that wants
    /// to surface an action or diagnostic in the log pane without
    /// going through a toast. Both MockHost and FFIHost implement it
    /// as `messages.append(_)` with trimming to the ~64 entry cap.
    func pushMessage(_ line: String)

    /// Last-in-first-out transient toast. Non-nil for a few seconds after
    /// an error/alert. ToastOverlay clears this to nil after its fade-out.
    var transientToast: ToastMessage? { get set }

    /// Called every ~100 ms by EventPoller. The real host ticks auto-advance
    /// and drains the FFI event buffer; the mock is a no-op. Returning an
    /// event array lets the poller dispatch them via `handle(event:)`.
    func tick() -> [MarginaliaEvent]

    /// Trigger an asset download. Returns immediately; progress surfaces
    /// through `inflightDownloads`. Unknown ids are a no-op.
    func installAsset(_ id: String)
    /// Remove a downloaded asset. No-op on the mock; live host deletes from
    /// the HF cache. Unknown or not-installed ids are no-ops.
    func uninstallAsset(_ id: String)
    /// Refresh the installed state of `installations` — cheap probe of the
    /// cache directory. Called after install/uninstall completes.
    func refreshInstallations() async

    /// Refresh the **catalog** itself from huggingface.co (network call,
    /// short timeout, falls back to the bundled manifest on failure).
    /// Triggered by the user from onboarding or the "Aggiorna lista voci"
    /// button in Settings. Mock host is a no-op that just flips
    /// `catalogRefreshInflight` briefly so the UI can be exercised.
    func refreshRemoteVoiceCatalog() async
    /// `true` while `refreshRemoteVoiceCatalog()` is in flight — the UI
    /// disables the button and shows a spinner.
    var catalogRefreshInflight: Bool { get }
    /// Last-attempt human-readable status ("Lista aggiornata: 28 voci",
    /// "Aggiornamento fallito: …"). Cleared at the next call.
    var catalogRefreshStatus: String? { get }

    /// Real-time audio levels from the AEC pipeline, fed to the waveform
    /// widget in the Sidebar footer. Empty when no AEC is running (non-Apple
    /// STT, or before the first mic frame). The mock synthesises a gentle
    /// oscillation so the bars aren't flat in the preview.
    var micLevels: [Float] { get }
    var ttsLevels: [Float] { get }

    /// JSON-encoded provider doctor report — rendered under the Settings
    /// page's "Diagnostica" section. Empty when the host can't produce one.
    func doctorReportJson() -> String

    /// Absolute path of the currently-configured Whisper ggml model, or nil
    /// if the user hasn't pointed `[stt.whisper] model_path` at anything.
    /// Used by the STT section to display "installato · path" with a
    /// Reveal-in-Finder button.
    var whisperModelPath: String? { get }

    /// Absolute path of the TTS audio cache directory. Used by Settings
    /// to compute the on-disk size and offer "reveal in Finder".
    var ttsCacheDir: String { get }

    /// Synthesize a short preview WAV for the given text in the given voice
    /// and return the absolute path. Used by the Voice preview play button
    /// in Settings. Throws if the current TTS backend is unavailable.
    func synthesizePreview(text: String, voice: String) async throws -> String

    /// Handle an event coming from the runtime — called by EventPoller.
    /// Default impl updates the Published state.
    func handle(event: MarginaliaEvent)

    /// Persist voice-command edits (doesn't go through apply_provider_spec).
    func saveVoiceCommands(_ commands: [VoiceCommand])

    /// Persist chunk size / STT debug toggle (local only).
    func saveAudioPrefs(chunkTargetChars: Int, sttDebug: Bool)
}

// MARK: — Mock host (drives the preview; also used by SwiftUI `#Preview` blocks)

@MainActor
public final class MockHost: MarginaliaHost, ObservableObject {
    public var currentSpec: ProviderSpec

    public let languages: [LangInfo] = [
        LangInfo(bcp47: "it-IT", display: "italiano",      voiceCount: 4),
        LangInfo(bcp47: "en-US", display: "English (US)",   voiceCount: 6),
        LangInfo(bcp47: "en-GB", display: "English (UK)",   voiceCount: 3),
        LangInfo(bcp47: "fr-FR", display: "français",       voiceCount: 2),
        LangInfo(bcp47: "es-ES", display: "español",        voiceCount: 2),
        LangInfo(bcp47: "ja-JP", display: "日本語",          voiceCount: 1),
    ]

    public let voices: [VoiceInfo] = [
        VoiceInfo(id: "if_sara",   display: "Sara",   lang: "it-IT", gender: .female, backend: "mlx", installed: true),
        VoiceInfo(id: "if_lucia",  display: "Lucia",  lang: "it-IT", gender: .female, backend: "mlx", installed: true),
        VoiceInfo(id: "im_nicola", display: "Nicola", lang: "it-IT", gender: .male,   backend: "mlx", installed: true),
        VoiceInfo(id: "im_marco",  display: "Marco",  lang: "it-IT", gender: .male,   backend: "mlx", installed: false),
        VoiceInfo(id: "af_bella",  display: "Bella",  lang: "en-US", gender: .female, backend: "mlx", installed: false),
        VoiceInfo(id: "am_adam",   display: "Adam",   lang: "en-US", gender: .male,   backend: "mlx", installed: false),
    ]

    public let sttEngines: [SttEngineInfo] = [
        SttEngineInfo(engineId: "apple",   name: "Apple Speech",    available: true,  reason: nil,
                      note: "richiede macOS Dictation attivo (Sistema → Tastiera → Dettatura)."),
        SttEngineInfo(engineId: "whisper", name: "Whisper (ggml)",  available: true,  reason: nil,
                      note: "modello small.bin (465 MB) installato — offline."),
    ]

    public let ttsBackends: [TtsBackendInfo] = [
        TtsBackendInfo(backendId: "mlx",    name: "Kokoro MLX",
                       sub: "Apple Silicon · Metal",    available: true,  reason: nil),
        TtsBackendInfo(backendId: "kokoro", name: "Kokoro ONNX",
                       sub: "fallback cross-platform",  available: false,
                       reason: "ONNX Runtime non installato"),
    ]

    @Published public var voiceCommands: [VoiceCommand] = [
        VoiceCommand(action: "pause",        label: "Metti in pausa",          triggers: ["pausa", "ferma"]),
        VoiceCommand(action: "resume",       label: "Riprendi",                triggers: ["riprendi", "continua"]),
        VoiceCommand(action: "next",         label: "Chunk successivo",        triggers: ["avanti", "prossimo"]),
        VoiceCommand(action: "back",         label: "Chunk precedente",        triggers: ["indietro"]),
        VoiceCommand(action: "repeat",       label: "Ripeti chunk",            triggers: ["ripeti"]),
        VoiceCommand(action: "stop",         label: "Ferma e riavvolgi",       triggers: ["stop", "basta"]),
        VoiceCommand(action: "next_chapter", label: "Capitolo successivo",     triggers: ["prossimo capitolo", "capitolo avanti"]),
        VoiceCommand(action: "prev_chapter", label: "Capitolo precedente",     triggers: ["capitolo indietro", "capitolo precedente"]),
        VoiceCommand(action: "bookmark",     label: "Salva posizione",         triggers: ["segna", "segnalibro"]),
        VoiceCommand(action: "note",         label: "Detta una nota",          triggers: ["nota", "appunto"]),
        VoiceCommand(action: "where",        label: "Leggi posizione",         triggers: ["dove sono", "posizione"]),
    ]

    @Published public var installations: [InstallableAsset] = [
        InstallableAsset(id: "mlx_it",        label: "Kokoro MLX — motore TTS",          size: "74 MB",  installed: true,  removable: false, category: "tts_core"),
        InstallableAsset(id: "whisper_small", label: "Whisper small (STT multilingua)",  size: "465 MB", installed: true,  removable: true,  category: "stt"),
        InstallableAsset(id: "voice_sara",    label: "Sara (femminile)",                 size: "0.5 MB", installed: true,  removable: true,  category: "voice", language: "it-IT"),
        InstallableAsset(id: "voice_lucia",   label: "Lucia (femminile)",                size: "0.5 MB", installed: true,  removable: true,  category: "voice", language: "it-IT"),
        InstallableAsset(id: "voice_nicola",  label: "Nicola (maschile)",                size: "0.5 MB", installed: true,  removable: true,  category: "voice", language: "it-IT"),
        InstallableAsset(id: "voice_marco",   label: "Marco (maschile)",                 size: "0.5 MB", installed: false, removable: false, category: "voice", language: "it-IT"),
        InstallableAsset(id: "voice_bella",   label: "Bella (femminile)",                size: "0.5 MB", installed: false, removable: false, category: "voice", language: "en-US"),
        InstallableAsset(id: "onnx",          label: "ONNX Runtime (TTS fallback)",      size: "34 MB",  installed: false, removable: false, category: "tts_core"),
        InstallableAsset(id: "pdfium",        label: "PDFium (import PDF)",              size: "68 MB",  installed: false, removable: false, category: "importer"),
    ]
    @Published public var inflightDownloads: [String: InstallUiState] = [:]
    @Published public var catalogRefreshInflight: Bool = false
    @Published public var catalogRefreshStatus: String? = nil

    /// Mock implementation: flips `catalogRefreshInflight` for ~1s and sets
    /// a fake success message. Lets the SwiftUI preview exercise the spinner
    /// without any network or FFI.
    public func refreshRemoteVoiceCatalog() async {
        await MainActor.run {
            self.catalogRefreshInflight = true
            self.catalogRefreshStatus = nil
        }
        try? await Task.sleep(nanoseconds: 800_000_000)
        await MainActor.run {
            self.catalogRefreshInflight = false
            self.catalogRefreshStatus = "Lista aggiornata: \(self.installations.count) voci."
        }
    }

    @Published public var chunkTargetChars: Int = 300
    @Published public var sttDebug: Bool = true
    @Published public var needsOnboarding: Bool = false
    @Published public var messages: [String] = []
    @Published public var transientToast: ToastMessage? = nil

    public func markOnboardingComplete() { needsOnboarding = false }

    /// Append a status line. Bounded to 64 entries, oldest-first drop.
    /// Lines prefixed with "Errore" also raise a transient error toast.
    public func pushMessage(_ m: String) {
        if messages.count >= 64 { messages.removeFirst() }
        messages.append(m)
        if m.hasPrefix("Errore") {
            transientToast = ToastMessage(text: m, kind: .error)
        }
    }

    // Reader-side mock state. Pre-populated with the prototype's sample
    // document so the preview shows something reasonable.
    @Published public var library: [LibraryEntry] = [
        LibraryEntry(id: "lib-1", title: "La montagna incantata", subtitle: "Thomas Mann",
                     progressPct: 34, chapterCount: 8,  chunkCount: 214, notes: 12, active: true),
        LibraryEntry(id: "lib-2", title: "Lettera a Giulia — v4", subtitle: "bozza",
                     progressPct: 88, chapterCount: 1,  chunkCount: 7,   notes: 1,  active: false),
        LibraryEntry(id: "lib-3", title: "Appunti sul Simposio", subtitle: "Platone",
                     progressPct: 12, chapterCount: 5,  chunkCount: 68,  notes: 4,  active: false),
        LibraryEntry(id: "lib-4", title: "Note al convegno", subtitle: "bozza",
                     progressPct: 56, chapterCount: 3,  chunkCount: 22,  notes: 7,  active: false),
        LibraryEntry(id: "lib-5", title: "Il giovane Holden", subtitle: "J.D. Salinger",
                     progressPct: 0,  chapterCount: 26, chunkCount: 312, notes: 0,  active: false),
        LibraryEntry(id: "lib-6", title: "Paesaggi della mente", subtitle: "saggio · v2",
                     progressPct: 22, chapterCount: 4,  chunkCount: 48,  notes: 3,  active: false),
    ]

    @Published public var currentSession: SessionState? = SessionState(
        sessionId: "s-1", documentId: "lib-1",
        documentTitle: "La montagna incantata",
        sectionIndex: 2, sectionCount: 8,
        sectionTitle: "il tempo in montagna",
        chunkIndex: 1, chunkText: ReadingMock.chunks[1].text,
        anchor: "c2", playbackState: .playing, notesCount: 12,
        voice: "if_sara"
    )

    @Published public var currentDocument: DocumentDoc? = DocumentDoc(
        documentId: "lib-1",
        title: "La montagna incantata",
        sections: [
            SectionDoc(index: 2, title: "il tempo in montagna",
                       chunks: ReadingMock.chunks),
        ]
    )

    @Published public var notes: [MarginNote] = ReadingMock.notes.filter { !$0.live }
    @Published public var liveNote: MarginNote? = ReadingMock.notes.first { $0.live }
    @Published public var synthesizingAnchor: String? = nil
    @Published public var ingestingSource: String? = nil
    @Published public var volume: Double = 1.0

    public init(spec: ProviderSpec = ProviderSpec(ttsBackend: "mlx", voice: "if_sara",
                                                  sttEngine: "apple", language: "it-IT")) {
        self.currentSpec = spec
    }

    // Reader actions — no-op in the mock, just nudges state.
    public func refreshLibrary() async { /* static list */ }
    public func openDocument(id: String) async throws { /* no-op */ }
    public func importFile(url: URL) async throws -> String {
        let newId = "lib-\(library.count + 1)"
        library.insert(LibraryEntry(
            id: newId, title: url.deletingPathExtension().lastPathComponent,
            subtitle: "importato", progressPct: 0, notes: 0, active: false
        ), at: 0)
        pushMessage("Importato: \(url.lastPathComponent)")
        return newId
    }
    public func importUrl(_ url: String) async throws -> String {
        let newId = "lib-url-\(library.count + 1)"
        library.insert(LibraryEntry(
            id: newId, title: url, subtitle: "URL", progressPct: 0, notes: 0, active: false
        ), at: 0)
        pushMessage("URL importato: \(url)")
        return newId
    }
    public func bookmark() async throws {
        guard let s = currentSession else {
            pushMessage("Nessuna sessione attiva."); return
        }
        pushMessage("Segnalibro · ch.\(s.sectionIndex + 1) chunk \(s.chunkIndex + 1)")
    }
    public func announcePosition() -> String? {
        guard let s = currentSession else { return nil }
        let pos = "capitolo \(s.sectionIndex + 1)/\(s.sectionCount) · chunk \(s.chunkIndex + 1)"
        pushMessage("Posizione: \(pos)")
        return pos
    }
    @discardableResult
    public func aecSetRenderReference(path: String) -> Bool { false }
    public func aecClearRenderReference() {}
    public func tick() -> [MarginaliaEvent] {
        // Animate a gentle mock waveform so the Sidebar footer visibly
        // breathes. Real AEC levels arrive from the FFIHost in production.
        let t = Date().timeIntervalSinceReferenceDate
        let mic = (0..<20).map { i in
            Float(abs(sin(t * 2 + Double(i) * 0.4)) * 0.35 + 0.05)
        }
        let tts = (0..<20).map { i in
            Float(abs(sin(t * 3 + Double(i) * 0.25)) * 0.5 + 0.08)
        }
        micLevels = mic
        ttsLevels = tts
        return []
    }

    /// Mock install: advance the row through queued → downloading → installed
    /// over ~2 s so the UX flow can be previewed without actual downloads.
    public func installAsset(_ id: String) {
        guard let idx = installations.firstIndex(where: { $0.id == id }),
              !installations[idx].installed else { return }
        inflightDownloads[id] = .queued
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 400_000_000)
            await MainActor.run { self?.inflightDownloads[id] = .downloading(fraction: nil) }
            try? await Task.sleep(nanoseconds: 1_500_000_000)
            await MainActor.run {
                guard let self = self else { return }
                self.inflightDownloads[id] = .installed
                if let j = self.installations.firstIndex(where: { $0.id == id }) {
                    self.installations[j].installed = true
                }
            }
        }
    }

    public func uninstallAsset(_ id: String) {
        guard let idx = installations.firstIndex(where: { $0.id == id }),
              installations[idx].removable else { return }
        installations[idx].installed = false
        inflightDownloads.removeValue(forKey: id)
    }

    public func refreshInstallations() async { /* no-op for mock */ }

    public func deleteDocument(id: String) async {
        library.removeAll { $0.id == id }
        if currentSession?.documentId == id {
            currentSession = nil
            currentDocument = nil
            notes = []
        }
    }

    public func openSourceInEditor(id: String) {
        // Mock: no actual file to open. Surface as a status message
        // so the SwiftUI preview shows the action fired.
        pushMessage("Mock: openSourceInEditor(\(id))")
    }

    public func reloadDocument(id: String) async throws {
        // Mock: just clear the needsReload flag if we had one.
        if let idx = library.firstIndex(where: { $0.id == id }) {
            let old = library[idx]
            library[idx] = LibraryEntry(
                id: old.id, title: old.title, subtitle: old.subtitle,
                progressPct: old.progressPct,
                chapterCount: old.chapterCount, chunkCount: old.chunkCount,
                notes: old.notes, active: old.active,
                sourcePath: old.sourcePath, needsReload: false
            )
        }
        pushMessage("Mock: reloadDocument(\(id))")
    }

    public func deleteNote(id: String) async {
        notes.removeAll { $0.id == id }
        if liveNote?.id == id { liveNote = nil }
    }

    public func updateNote(id: String, text: String) async throws {
        if let idx = notes.firstIndex(where: { $0.id == id }) {
            let old = notes[idx]
            notes[idx] = MarginNote(
                id: old.id, chunkId: old.chunkId, when: old.when,
                quote: old.quote, body: text, duration: old.duration,
                status: "modificata", live: old.live
            )
        }
    }

    public func exportNotesMarkdown() -> String {
        Self.renderNotesMarkdown(
            docTitle: currentSession?.documentTitle ?? "Documento",
            notes: notes
        )
    }

    public func exportBackup(path: String) async throws {
        // Mock: write a placeholder marker so preview targets can test
        // the menu flow without a real runtime underneath.
        try "Marginalia mock backup\n".write(toFile: path, atomically: true, encoding: .utf8)
    }

    public func importBackup(path: String) async throws {
        // Mock: no-op. Preview never has data to restore.
    }

    /// Shared markdown formatter used by both the mock and the live host
    /// so export output looks identical across targets.
    static func renderNotesMarkdown(docTitle: String, notes: [MarginNote]) -> String {
        var out = "# \(docTitle) — Note\n\n"
        if notes.isEmpty {
            out += "_(Nessuna nota per questo documento.)_\n"
            return out
        }
        for n in notes {
            out += "## \(n.when.isEmpty ? "nota" : n.when)"
            if !n.status.isEmpty { out += " — \(n.status)" }
            out += "\n\n"
            if !n.quote.isEmpty {
                out += "> \(n.quote)\n\n"
            }
            out += "\(n.body)\n\n"
            if !n.duration.isEmpty {
                out += "_durata: \(n.duration)_\n\n"
            }
            out += "---\n\n"
        }
        return out
    }

    @Published public var micLevels: [Float] = []
    @Published public var ttsLevels: [Float] = []

    public var whisperModelPath: String? {
        // Mock points at the canonical install location produced by
        // `make bootstrap-whisper`; real host reads from marginalia.toml.
        "models/stt/whisper/ggml-small.bin"
    }

    public var ttsCacheDir: String {
        ".marginalia/tts-cache"
    }

    public func synthesizePreview(text: String, voice: String) async throws -> String {
        // Mock: no actual synthesis available; we just simulate a delay and
        // return a path that the caller handles gracefully (plays nothing).
        try await Task.sleep(for: .milliseconds(500))
        pushMessage("Anteprima: \"\(text.prefix(32))…\" con \(voice) (mock)")
        return ""  // empty path = nothing to play
    }

    public func doctorReportJson() -> String {
        #"""
        {
          "tts": {"provider": "kokoro-mlx", "voice": "if_sara", "ready": true},
          "stt": {"provider": "apple", "language": "it-IT", "ready": true},
          "playback": {"provider": "rodio", "ready": true},
          "cache": {"path": ".marginalia/tts-cache", "size_mb": 142},
          "runtime": {"arch": "aarch64", "platform": "macOS 26.1", "metal": 4}
        }
        """#
    }
    public func pause() async throws {
        if let s = currentSession { currentSession = withPlayback(s, .paused) }
    }
    public func resume() async throws {
        if let s = currentSession { currentSession = withPlayback(s, .playing) }
    }
    public func stop() async throws { currentSession = nil }
    public func next() async throws { bumpChunk(by: +1) }
    public func back() async throws { bumpChunk(by: -1) }
    public func repeatCurrent() async throws { /* no-op */ }
    public func nextChapter() async throws { /* no-op */ }
    public func previousChapter() async throws { /* no-op */ }
    public func seekToChunk(section: Int, chunk: Int) async throws {
        // Mock just snaps the current session to the target.
        guard var s = currentSession else { return }
        s = SessionState(
            sessionId: s.sessionId, documentId: s.documentId,
            documentTitle: s.documentTitle,
            sectionIndex: section, sectionCount: s.sectionCount,
            sectionTitle: s.sectionTitle, chunkIndex: chunk,
            chunkText: s.chunkText, anchor: "c\(section)-\(chunk)",
            playbackState: s.playbackState, notesCount: s.notesCount,
            voice: s.voice
        )
        currentSession = s
    }
    public func seekToChunkPaused(section: Int, chunk: Int) async throws {
        try await seekToChunk(section: section, chunk: chunk)
        // Mock has no real audio engine — flip the snapshot's state to
        // .paused so the toolbar reflects the silent-seek semantics.
        guard var s = currentSession else { return }
        s = SessionState(
            sessionId: s.sessionId, documentId: s.documentId,
            documentTitle: s.documentTitle,
            sectionIndex: s.sectionIndex, sectionCount: s.sectionCount,
            sectionTitle: s.sectionTitle, chunkIndex: s.chunkIndex,
            chunkText: s.chunkText, anchor: s.anchor,
            playbackState: .paused, notesCount: s.notesCount,
            voice: s.voice
        )
        currentSession = s
    }

    public func createNote(text: String) async throws {
        let anchor = currentSession?.anchor ?? ""
        let newNote = MarginNote(
            id: "m-\(UUID().uuidString.prefix(8))",
            chunkId: anchor, when: "ora", quote: "",
            body: text, duration: "", status: "", live: false
        )
        notes.insert(newNote, at: 0)
        pushMessage("Nota aggiunta")
    }

    public func clearLiveNote() { liveNote = nil }

    public func cancelPendingDictation() {
        // Mock has no real dictation thread to abort; just drop the
        // live card so the preview UX matches the real flow.
        liveNote = nil
    }

    /// Mock dictation: pop a live note placeholder, then resolve it after
    /// ~2 s with a canned transcript so the preview shows the full flow.
    public func startDictation() {
        liveNote = MarginNote(
            id: "live", chunkId: currentSession?.anchor ?? "",
            when: "ora", quote: "", body: "…",
            duration: "0:00", status: "", live: true
        )
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: 2_000_000_000)
            await MainActor.run {
                guard let self = self else { return }
                let transcript = "Questa è una nota di prova, dettata a voce."
                self.liveNote = MarginNote(
                    id: "live", chunkId: self.currentSession?.anchor ?? "",
                    when: "ora", quote: "", body: transcript,
                    duration: "0:02", status: "applicato", live: true
                )
            }
        }
    }

    private func bumpChunk(by d: Int) {
        guard let s = currentSession else { return }
        let chunks = currentDocument?.sections.first?.chunks ?? []
        let newIdx = max(0, min(chunks.count - 1, s.chunkIndex + d))
        currentSession = SessionState(
            sessionId: s.sessionId, documentId: s.documentId,
            documentTitle: s.documentTitle,
            sectionIndex: s.sectionIndex, sectionCount: s.sectionCount,
            sectionTitle: s.sectionTitle,
            chunkIndex: newIdx,
            chunkText: chunks[safe: newIdx]?.text ?? s.chunkText,
            anchor: chunks[safe: newIdx]?.id ?? s.anchor,
            playbackState: s.playbackState, notesCount: s.notesCount,
            voice: s.voice
        )
    }

    public func handle(event: MarginaliaEvent) { /* no-op in mock */ }

    public func apply(spec: ProviderSpec) async throws -> ApplyReport {
        try await Task.sleep(for: .milliseconds(700))
        let languageChanged = spec.language != currentSpec.language
        let sttSwapped = spec.sttEngine != currentSpec.sttEngine || languageChanged
        let ttsSwapped = spec.voice != currentSpec.voice || spec.ttsBackend != currentSpec.ttsBackend
        currentSpec = spec
        return ApplyReport(ttsSwapped: ttsSwapped, sttSwapped: sttSwapped,
                           languageChanged: languageChanged, elapsedMs: 700)
    }

    public func saveVoiceCommands(_ commands: [VoiceCommand]) {
        voiceCommands = commands
    }

    public func saveAudioPrefs(chunkTargetChars: Int, sttDebug: Bool) {
        self.chunkTargetChars = chunkTargetChars
        self.sttDebug = sttDebug
    }
}

private func withPlayback(_ s: SessionState, _ p: PlaybackState) -> SessionState {
    SessionState(
        sessionId: s.sessionId, documentId: s.documentId,
        documentTitle: s.documentTitle,
        sectionIndex: s.sectionIndex, sectionCount: s.sectionCount,
        sectionTitle: s.sectionTitle,
        chunkIndex: s.chunkIndex, chunkText: s.chunkText,
        anchor: s.anchor,
        playbackState: p, notesCount: s.notesCount,
        voice: s.voice
    )
}

extension Array {
    fileprivate subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
