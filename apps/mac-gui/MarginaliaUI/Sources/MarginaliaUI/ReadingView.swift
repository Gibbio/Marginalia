import SwiftUI
#if canImport(AVFoundation)
import AVFoundation
#endif

// Demo chunks / notes (mock — the real app passes domain types).
public enum ReadingMock {
    public static let chunks: [ReadingChunk] = [
        ReadingChunk(id: "c1", index: 0,
            text: "Il tempo, nell'alta montagna, non è il tempo della pianura. Si dilata, si contrae, talvolta sembra fermarsi del tutto, come se l'aria rarefatta ne modificasse la sostanza stessa."),
        ReadingChunk(id: "c2", index: 1,
            text: "Hans Castorp osservava la neve cadere oltre il vetro, e pensava — non senza un certo stupore — che erano passate già sette settimane dal suo arrivo, sette settimane che egli aveva contato come giorni, e che ora, al solo ricordarle, gli parevano un istante."),
        ReadingChunk(id: "c3", index: 2,
            text: "Ma forse, pensò, non è la durata a contare, quanto la qualità del tempo vissuto. Una settimana in pianura poteva dissolversi senza lasciare traccia; mentre un solo pomeriggio quassù, trascorso a guardare il cielo cambiare colore sopra i larici, poteva pesare come un anno intero."),
        ReadingChunk(id: "c4", index: 3,
            text: "Joachim, suo cugino, rideva di queste sue meditazioni. \u{00AB}Tu filosofeggi, Hans\u{00BB}, diceva, \u{00AB}come fanno tutti i principianti. Tra sei mesi avrai smesso\u{00BB}."),
        ReadingChunk(id: "c5", index: 4,
            text: "Qui, in alto, dove persino il \u{00AB}sanatorio\u{00BB} pareva sospeso tra due cieli, le parole perdevano il loro peso quotidiano e ne assumevano uno nuovo, più lento, più pieno."),
        ReadingChunk(id: "c6", index: 5,
            text: "E pure nel ridere c'era una malinconia sottile, perché Joachim stesso — il giovane ufficiale che sognava di tornare al reggimento — aveva smesso, a forza, di contare i giorni."),
    ]

    public static let notes: [MarginNote] = [
        MarginNote(id: "n0", chunkId: "c2", when: "poco fa",
                   quote: "…sette settimane dal suo arrivo…",
                   body: "Qui il tempo è trattato come uno spazio che si può attraversare. Rendilo più sensoriale — fai sentire la lentezza.",
                   duration: "0:14", status: "rielaborato", live: true),
        MarginNote(id: "n1", chunkId: "c5", when: "10 min fa",
                   quote: "\u{00AB}sanatorio\u{00BB}",
                   body: "metafora o luogo reale? Mann gioca sul doppio senso per tutta la prima parte.",
                   duration: "0:21", status: ""),
        MarginNote(id: "n2", chunkId: "c3", when: "ieri",
                   quote: "il cielo cambiare colore",
                   body: "Confronta con Proust — la stessa attenzione al dettaglio atmosferico, ma qui più silenziosa.",
                   duration: "0:33", status: "applicato"),
        MarginNote(id: "n3", chunkId: "c4", when: "2 gg fa",
                   quote: "\u{00AB}Tu filosofeggi, Hans\u{00BB}",
                   body: "Joachim come contrappunto razionale. Mantieni la leggerezza dello scherzo.",
                   duration: "0:11", status: ""),
    ]
}

/// Pool of one-line tips shown in the margin panel footer. Rotates
/// every ~7s via TimelineView so the user discovers shortcuts
/// without us cluttering the UI with persistent help labels. Top-
/// level so it's reachable from the generic ReadingView (no static
/// stored properties allowed in generic types).
fileprivate let marginPanelHintKeys: [String] = [
    "reading.hint.note_hover",
    "reading.hint.note_click",
    "reading.hint.chunk_dblclick",
    "reading.hint.voice_note",
    "reading.hint.voice_control",
    "reading.hint.add_button",
    "reading.hint.note_edit",
    "reading.hint.settings",
]

// MARK: — PreferenceKeys for link-overlay geometry

struct ChunkFrame: Equatable { let id: String; let rect: CGRect }
struct NoteFrame: Equatable { let id: String; let rect: CGRect }

/// Key for the `.task(id:)` that drives scroll-to-current-chunk. Changing
/// either the document id or the anchor retriggers the task, so both a
/// mid-session advance and a first-paint restore land the view on the
/// right chunk.
struct ScrollTarget: Hashable {
    let docId: String?
    let anchor: String?
}

struct ChunkFramesKey: PreferenceKey {
    static let defaultValue: [ChunkFrame] = []
    static func reduce(value: inout [ChunkFrame], nextValue: () -> [ChunkFrame]) {
        value.append(contentsOf: nextValue())
    }
}

struct NoteFramesKey: PreferenceKey {
    static let defaultValue: [NoteFrame] = []
    static func reduce(value: inout [NoteFrame], nextValue: () -> [NoteFrame]) {
        value.append(contentsOf: nextValue())
    }
}

// MARK: — Reading view

public enum ReadingMode: String, Hashable, CaseIterable {
    /// Book mode (default): serif Cormorant italic, generous leading,
    /// justified-ish, the "read-aloud companion" feel.
    case book
    /// Text mode: sans-serif, tighter leading, literal paragraph flow.
    /// Useful when you want to scan the document structurally rather than
    /// settle into reading it.
    case text
    /// Technical mode: monospace, compact, no italics — for code, specs,
    /// structured documents where vertical rhythm matters more than warmth.
    case technical
}

/// Reading preferences persisted via `@AppStorage` across launches.
/// Keys are namespaced so they don't collide with other apps' defaults.
public enum ReadingPrefs {
    public static let modeKey  = "com.gibbio.marginalia.readingMode"
    public static let scaleKey = "com.gibbio.marginalia.readingScale"
    public static let scaleMin: Double = 0.75
    public static let scaleMax: Double = 1.60
}

public struct ReadingView<Host: MarginaliaHost>: View {
    public var accent: Accent
    @ObservedObject private var host: Host

    @State private var hoverId: String? = nil
    @State private var chunkFrames: [String: CGRect] = [:]
    @State private var noteFrames: [String: CGRect] = [:]
    @State private var containerWidth: CGFloat = 0
    /// Id of the note whose audio is currently being prepared OR
    /// playing. Drives the play/stop icon toggle on the note card and
    /// acts as a cancellation token: a slow TTS synth that returns
    /// for an id no longer matching here drops its result rather than
    /// playing for a vanished or replaced note.
    @State private var currentPlayingNoteId: String? = nil
    /// Active AVAudioPlayer for note playback. Held so a second tap on
    /// the same note (or a delete) can `stop()` the audio mid-playback
    /// instead of having to wait for it to drain.
    @State private var notePlayer: AVAudioPlayer? = nil
    /// Per-bucket peak amplitudes sampled from the playing note's audio
    /// file (WAV / FLAC). Empty when no note is playing — Waveform then
    /// falls back to its dim sine, which is fine because the bar is also
    /// idle. Populated alongside `notePlayer` in `startPlayingNote`.
    @State private var notePlayerLevels: [Float] = []
    /// Non-nil while the "+ aggiungi nota" sheet is open. The sheet
    /// pauses playback on present and lets the user type (or trigger
    /// voice dictation) — on save, runtime.createNote attaches the text
    /// to whatever chunk is the current reading position.
    @State private var composingNote: Bool = false
    @State private var noteDraft: String = ""
    /// Persisted across launches. AppStorage with a RawRepresentable enum
    /// requires a non-optional default — ReadingMode.book is used when the
    /// stored value is absent or unparseable.
    @AppStorage(ReadingPrefs.modeKey) private var readingMode: ReadingMode = .book
    /// Per-user text-size multiplier, persisted. 1.0 = design default.
    @AppStorage(ReadingPrefs.scaleKey) private var fontScale: Double = 1.0

    public init(accent: Accent, host: Host) {
        self.accent = accent
        self._host = ObservedObject(initialValue: host)
    }

    /// All sections of the currently-loaded document, in document order.
    /// The reading column renders every section inline (title + chunks)
    /// rather than one chapter at a time — the user explicitly wants to
    /// see the whole text with headings as navigational markers, not a
    /// paginated "Capitolo N" view.
    private var sections: [SectionDoc] {
        host.currentDocument?.sections ?? []
    }

    private var notes: [MarginNote] {
        var all = host.notes
        // Insert the live (in-flight or just-saved) note ONLY if it
        // isn't already in the persisted list. After
        // `voiceNoteTranscribed` fires we do both `liveNote = saved` AND
        // `refreshNotes` — without this guard the user would see the
        // same note twice for the ~3s display window of the live card.
        if let live = host.liveNote,
           !all.contains(where: { $0.id == live.id }) {
            all.insert(live, at: 0)
        }
        return all
    }

    public var body: some View {
        VStack(spacing: 0) {
            Toolbar(
                accent: accent,
                title: host.currentSession?.documentTitle ?? "",
                playbackState: host.currentSession?.playbackState ?? .idle,
                synthesizing: host.synthesizingAnchor != nil
            )
            Divider().frame(height: 1).overlay(Tokens.line)
            mainArea
        }
        .background(Tokens.bg)
        .sheet(isPresented: $composingNote) {
            NoteComposeSheet(
                accent: accent,
                host: host,
                text: $noteDraft,
                anchorLabel: noteAnchorLabel,
                onClose: { composingNote = false }
            )
        }
    }

    /// Human-readable "capitolo N · chunk M" label shown at the top of
    /// the compose sheet so the user knows which chunk the note will
    /// anchor to. Empty when there's no session.
    private var noteAnchorLabel: String {
        guard let s = host.currentSession else { return "" }
        return "capitolo \(s.sectionIndex + 1) · chunk \(s.chunkIndex + 1)"
    }

    /// Pause playback and open the compose sheet. Pausing first matches
    /// the user's intent: while they're typing or dictating, the TTS
    /// voice shouldn't keep reading over them.
    private func openComposeNote() {
        Task {
            if host.currentSession?.playbackState == .playing {
                try? await host.pause()
            }
            noteDraft = ""
            await MainActor.run { composingNote = true }
        }
    }

    /// Play a saved note back. When the note carries a recorded WAV
    /// (dictation path — `audioReference` non-nil), we play the user's
    /// own voice via rodio. Otherwise we fall back to TTS-synthesizing
    /// the transcript in the current voice — typed notes, bookmarks,
    /// and dictations that were edited afterwards (raw_audio_path was
    /// cleared by `update_note` to keep transcript + audio coherent).
    ///
    /// We re-lookup the note from `host.notes` by id at play time so
    /// a fresh edit (which flipped `audioReference` to nil) isn't
    /// shadowed by a stale capture in the `ForEach` closure.
    /// Toggle playback of a note: tap once to start, tap again (same
    /// note) to stop, tap a different note to switch over without the
    /// two voices overlapping. Resets `currentPlayingNoteId` and the
    /// `notePlayer` together so the UI's play/stop glyph and the
    /// audible state stay coherent.
    private func playNote(_ noteId: String) {
        // Tap on the note that's already playing → stop.
        if currentPlayingNoteId == noteId {
            stopNotePlayback()
            return
        }
        // Different note: silence the previous one before kicking off.
        stopNotePlayback()

        let note = host.notes.first(where: { $0.id == noteId })
            ?? (host.liveNote?.id == noteId ? host.liveNote : nil)
        guard let note else {
            host.pushMessage("Play nota: id \(noteId.prefix(8)) non trovato")
            return
        }
        currentPlayingNoteId = noteId

        if let path = note.audioReference, !path.isEmpty,
           FileManager.default.fileExists(atPath: path) {
            host.pushMessage("Play nota: audio registrato")
            startPlayingNote(noteId: noteId, path: path)
            return
        }
        let text = note.body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else {
            host.pushMessage("Play nota: corpo vuoto, niente da leggere")
            currentPlayingNoteId = nil
            return
        }
        let voice = host.currentSpec.voice
        host.pushMessage("Play nota: TTS con \(voice)")
        Task { [weak host] in
            guard let host else { return }
            do {
                let path = try await host.synthesizePreview(text: text, voice: voice)
                await MainActor.run {
                    // Cancellation gate: stale tap (different / deleted note)?
                    guard self.currentPlayingNoteId == noteId else { return }
                    let stillExists = host.notes.contains(where: { $0.id == noteId })
                        || host.liveNote?.id == noteId
                    guard stillExists else {
                        host.pushMessage("Play nota: cancellata mentre sintetizzavo, salto")
                        self.currentPlayingNoteId = nil
                        return
                    }
                    if path.isEmpty {
                        host.pushMessage("Play nota: TTS path vuoto")
                        self.currentPlayingNoteId = nil
                        return
                    }
                    self.startPlayingNote(noteId: noteId, path: path)
                }
            } catch {
                await MainActor.run {
                    host.pushMessage("Errore TTS nota: \(error.localizedDescription)")
                    self.currentPlayingNoteId = nil
                }
            }
        }
    }

    /// Open `path` in an `AVAudioPlayer`, start playback, and schedule
    /// a state reset for when the audio finishes naturally. Replaces
    /// any previously-active player. AVAudioPlayer handles WAV (real
    /// dictation recordings) and FLAC (TTS output) on macOS 14+.
    private func startPlayingNote(noteId: String, path: String) {
        let url = URL(fileURLWithPath: path)
        guard let player = try? AVAudioPlayer(contentsOf: url) else {
            host.pushMessage("Play nota: impossibile aprire audio")
            currentPlayingNoteId = nil
            return
        }
        player.prepareToPlay()
        notePlayer = player
        // Sample real peaks from the file so the footer waveform shows
        // the actual envelope of THIS note (matches the SettingsView
        // voice-preview pattern). Sized to NoteCard's bar count.
        notePlayerLevels = ReadingView.waveformLevels(fromAudioFile: path, buckets: 32)
        // AVAudioPlayer plays through the system output WITHOUT going
        // through the rodio HostPlaybackEngine, so the chunk-playback's
        // AEC render callback never fires for this audio. We feed the
        // reference manually here — AEC3 will subtract the note signal
        // from the mic capture so a body containing "nota" or any other
        // voice trigger doesn't auto-fire commands when the speaker
        // echoes back. No-op on STT engines without an AEC pipeline.
        host.aecSetRenderReference(path: path)
        let duration = player.duration
        player.play()
        // Auto-clear when audio finishes naturally. ReadingView is a
        // struct (no `weak self`); the closure captures the property
        // wrappers' projected references (@State binds), which keep
        // pointing at the same backing storage even if the view is
        // re-created. The id-match guard handles the case where the
        // user has moved on to a different note in the meantime.
        Task {
            try? await Task.sleep(for: .seconds(max(duration, 0.5) + 0.3))
            await MainActor.run {
                if currentPlayingNoteId == noteId {
                    notePlayer = nil
                    notePlayerLevels = []
                    currentPlayingNoteId = nil
                    host.aecClearRenderReference()
                }
            }
        }
    }

    /// Halt any active note playback and clear the play/stop glyph
    /// state. Idempotent — safe to call when nothing is playing.
    private func stopNotePlayback() {
        notePlayer?.stop()
        notePlayer = nil
        notePlayerLevels = []
        currentPlayingNoteId = nil
        host.aecClearRenderReference()
    }

    /// Read a WAV/FLAC at `path` and downsample its absolute amplitudes
    /// into `buckets` peak values (0…1). Mirrors the helper in
    /// SettingsView so NoteCard's footer waveform reflects the real
    /// envelope of the playing note rather than a static sine.
    fileprivate static func waveformLevels(fromAudioFile path: String, buckets: Int) -> [Float] {
        let url = URL(fileURLWithPath: path)
        guard let file = try? AVAudioFile(forReading: url) else { return [] }
        let format = file.processingFormat
        let frameCount = AVAudioFrameCount(file.length)
        guard frameCount > 0,
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount)
        else { return [] }
        do { try file.read(into: buffer) } catch { return [] }
        guard let channels = buffer.floatChannelData else { return [] }
        let count = Int(buffer.frameLength)
        guard count > 0 else { return [] }
        let samples = UnsafeBufferPointer(start: channels[0], count: count)
        let bucketSize = max(1, count / buckets)
        var levels: [Float] = []
        levels.reserveCapacity(buckets)
        for i in 0..<buckets {
            let start = i * bucketSize
            let end = min(count, start + bucketSize)
            var peak: Float = 0
            for j in start..<end {
                let v = abs(samples[j])
                if v > peak { peak = v }
            }
            levels.append(min(peak, 1.0))
        }
        return levels
    }

    /// Parse a note's anchor ("section:N/chunk:M") and seek the reader
    /// to that position. Single-click on the note jumps the reader to
    /// the chunk in PAUSE — the user can review silently or hit play.
    /// Routes through `seekToChunkPaused` (FFI variant that pauses
    /// atomically post-seek under the same runtime lock) so we never
    /// emit the brief audio burst the regular `seekToChunk` would
    /// (its `replay_session_at_position` always auto-plays via the
    /// rodio sink). Silently no-ops on malformed anchors.
    private func seekToNote(_ note: MarginNote) {
        guard let (section, chunk) = parseNoteAnchor(note.chunkId) else { return }
        Task {
            try? await host.seekToChunkPaused(section: section, chunk: chunk)
        }
    }

    /// Double-click variant: same as `seekToNote` but immediately
    /// resumes reading after the seek so the user can audit the
    /// passage out loud with one gesture.
    private func seekToNoteAndPlay(_ note: MarginNote) {
        guard let (section, chunk) = parseNoteAnchor(note.chunkId) else { return }
        Task {
            try? await host.seekToChunk(section: section, chunk: chunk)
            try? await host.resume()
        }
    }

    /// Decode "section:N/chunk:M" → (N, M). Returns nil on any other
    /// shape so callers can no-op cleanly.
    private func parseNoteAnchor(_ anchor: String) -> (Int, Int)? {
        let parts = anchor.split(separator: "/")
        guard parts.count == 2,
              let section = Int(parts[0].split(separator: ":").last ?? ""),
              let chunk = Int(parts[1].split(separator: ":").last ?? "")
        else { return nil }
        return (section, chunk)
    }

    /// Uppercased document title — the kicker line above the main text.
    /// Empty if we somehow render without a document (MarginaliaWindow
    /// gates this, but defense-in-depth).
    private var headerKicker: String {
        (host.currentDocument?.title ?? "").uppercased()
    }

    private var mainArea: some View {
        GeometryReader { geom in
            ZStack(alignment: .topLeading) {
                HStack(spacing: 0) {
                    readingColumn
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                    Divider().frame(width: 1).overlay(Tokens.line)
                    marginPanel
                        .frame(width: 340)
                }
                linkOverlay
                    .allowsHitTesting(false)
            }
            // Coordinate space defined HERE (on the ZStack), not on the
            // outer ReadingView. Chunks and notes report their rects in
            // this space; `linkOverlay` draws into this same space — so
            // the Path's (x,y) land exactly where the source rects are.
            // Previously the overlay was offset by the Toolbar height
            // (~53 pt) because `reading-root` was anchored higher up.
            .coordinateSpace(name: "reading-root")
            .onAppear { containerWidth = geom.size.width }
            .onChange(of: geom.size) { _, new in containerWidth = new.width }
        }
        .onPreferenceChange(ChunkFramesKey.self) { arr in
            var map: [String: CGRect] = [:]
            for c in arr { map[c.id] = c.rect }
            chunkFrames = map
        }
        .onPreferenceChange(NoteFramesKey.self) { arr in
            var map: [String: CGRect] = [:]
            for n in arr { map[n.id] = n.rect }
            noteFrames = map
        }
    }

    // Reading column
    private var readingColumn: some View {
        let anchoredIds = Set(notes.map { $0.chunkId })
        return ZStack {
            // Ambient radial gradient behind the text.
            RadialGradient(
                colors: [accent.main.opacity(0.12), .clear],
                center: .center, startRadius: 50, endRadius: 500
            )
            .blur(radius: 28)

            ScrollViewReader { proxy in
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        // Document-level kicker: title uppercased as a
                        // thin top label, then all sections flow inline
                        // below with their own titles.
                        Text(headerKicker)
                            .font(.mono(10))
                            .tracking(2)
                            .foregroundStyle(Tokens.textFaint)
                            .padding(.bottom, headerPaddingBottom)

                        // Empty-document guard: some PDFs (scans without OCR,
                        // or DRM-protected files) extract zero chunks. Show
                        // a gentle explanation rather than a blank area.
                        if sections.allSatisfy({ $0.chunks.isEmpty }) {
                            VStack(alignment: .leading, spacing: 10) {
                                Text(T("reading.empty.kicker"))
                                    .font(.mono(10)).tracking(1.5)
                                    .foregroundStyle(Tokens.textFaint)
                                Text(T("reading.empty.body"))
                                    .font(.serif(15, italic: true))
                                    .foregroundStyle(Tokens.textDim)
                                    .lineSpacing(4)
                                    .frame(maxWidth: 520, alignment: .leading)
                            }
                            .padding(.top, 20)
                            .frame(maxWidth: 640, alignment: .leading)
                        } else {
                            VStack(alignment: .leading, spacing: 0) {
                                ForEach(sections, id: \.index) { section in
                                    sectionBlock(section, anchoredIds: anchoredIds)
                                }
                            }
                            // Wider reading measure — the old 640 cap left
                            // big unused bands on either side even when the
                            // container padding was already tight. 760 gives
                            // a comfortable line length for 17pt serif.
                            .frame(maxWidth: 760, alignment: .leading)
                        }
                    }
                    .padding(.horizontal, 48)
                    .padding(.top, 20)
                    .padding(.bottom, 48)
                }
                .scrollIndicators(.hidden)
                // Scroll to the active chunk whenever (a) the anchor
                // changes mid-session (auto-advance, seek, next/back)
                // or (b) the view first appears on a restored session —
                // `.onChange` alone misses case (b) because it doesn't
                // fire at mount, leaving the user looking at chunk 0
                // instead of wherever they left off.
                //
                // Keyed on both doc id AND anchor so a restart that
                // loads the document asynchronously (session snapshot
                // arrives before `refreshDocumentView` populates the
                // sections) re-fires once the chunks are actually in
                // the layout tree. Brief sleep gives the ScrollView
                // one frame to measure content before scrollTo targets
                // it — without it, scrollTo on first render can be a
                // no-op against a not-yet-realized child view.
                .task(id: ScrollTarget(
                    docId: host.currentDocument?.documentId,
                    anchor: host.currentSession?.anchor
                )) {
                    guard let anchor = host.currentSession?.anchor,
                          host.currentDocument != nil else { return }
                    try? await Task.sleep(for: .milliseconds(80))
                    withAnimation(.easeInOut(duration: 0.35)) {
                        proxy.scrollTo(anchor, anchor: .center)
                    }
                }
                .overlay(alignment: .bottomLeading) {
                    readingModeToggle
                        .padding(.leading, 22).padding(.bottom, 22)
                }
                .overlay(alignment: .bottomTrailing) {
                    fontSizeStepper
                        .padding(.trailing, 22).padding(.bottom, 22)
                }
            }
        }
    }

    /// Compact vertical "slider" on the bottom-right: up arrow to enlarge
    /// text, down to shrink. A center label shows the current scale as a
    /// percentage. Persisted across launches via AppStorage.
    private var fontSizeStepper: some View {
        VStack(spacing: 0) {
            Button(action: { stepFont(by: +0.05) }) {
                Image(systemName: "chevron.up")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(Tokens.textDim)
                    .frame(maxWidth: .infinity, minHeight: 26)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(T("reading.font.larger"))

            Rectangle()
                .fill(Tokens.line)
                .frame(maxWidth: .infinity, maxHeight: 1)
                .padding(.horizontal, 6)

            Text("\(Int(fontScale * 100))%")
                .font(.mono(8))
                .foregroundStyle(Tokens.textFaint)
                .frame(maxWidth: .infinity, minHeight: 18)

            Rectangle()
                .fill(Tokens.line)
                .frame(maxWidth: .infinity, maxHeight: 1)
                .padding(.horizontal, 6)

            Button(action: { stepFont(by: -0.05) }) {
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(Tokens.textDim)
                    .frame(maxWidth: .infinity, minHeight: 26)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(T("reading.font.smaller"))
        }
        // Narrow column — roughly the height of the mode toggle's buttons
        // so the two feel visually paired (one vertical, one horizontal).
        .frame(width: 28)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .fill(Color(hex: 0x1A1815).opacity(0.82))
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .strokeBorder(Tokens.line, lineWidth: 1)
                )
        )
        // Trackpad drag also adjusts: up = larger, down = smaller.
        // ~80 pt of drag covers the full 75%→160% range.
        .gesture(
            DragGesture(minimumDistance: 2)
                .onChanged { g in
                    let delta = Double(-g.translation.height) / 800.0
                    fontScale = min(ReadingPrefs.scaleMax,
                                    max(ReadingPrefs.scaleMin, fontScale + delta))
                }
        )
    }

    private func stepFont(by delta: Double) {
        fontScale = min(ReadingPrefs.scaleMax,
                        max(ReadingPrefs.scaleMin, fontScale + delta))
    }

    /// Segmented reading-mode toggle, bottom-left. Three modes, icon-only
    /// so the pill stays compact and the glyphs (SF Symbols) do the talking.
    private var readingModeToggle: some View {
        HStack(spacing: 2) {
            modeButton(.book,      icon: "book",                             label: T("reading.mode.book"))
            modeButton(.text,      icon: "text.alignleft",                   label: T("reading.mode.text"))
            modeButton(.technical, icon: "chevron.left.forwardslash.chevron.right",
                                    label: T("reading.mode.technical"))
        }
        .padding(3)
        .background(
            Capsule().fill(Color(hex: 0x1A1815).opacity(0.75))
                .overlay(Capsule().strokeBorder(Tokens.line, lineWidth: 1))
        )
    }

    @ViewBuilder
    private func modeButton(_ mode: ReadingMode, icon: String, label: String) -> some View {
        let selected = readingMode == mode
        Button(action: { readingMode = mode }) {
            Image(systemName: icon)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(selected ? Tokens.bg : Tokens.textDim)
                .frame(width: 28, height: 22)
                .background(
                    Capsule().fill(selected ? accent.main : Color.clear)
                )
                // Without `contentShape`, SwiftUI only hit-tests on the
                // opaque pixels of the SF Symbol — the user has to land
                // on the glyph itself, not the surrounding capsule.
                // Forcing a Rectangle hit area makes the whole 28×22
                // frame tappable.
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(String(format: T("reading.mode.help"), label))
        .accessibilityLabel(String(format: T("reading.mode.help"), label))
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    /// Multiplier the fontScale slider drives, clamped to safety.
    private var scale: CGFloat {
        CGFloat(max(ReadingPrefs.scaleMin, min(ReadingPrefs.scaleMax, fontScale)))
    }

    /// Gap between the document-title kicker (uppercased mono line at the
    /// very top of the reading column) and whatever comes next. Intentionally
    /// tight — the first section title below has its own top padding, so
    /// adding more here would double-stack the whitespace.
    private var headerPaddingBottom: CGFloat {
        switch readingMode {
        case .book:      return 8
        case .text:      return 6
        case .technical: return 4
        }
    }

    /// Font size for inline section titles (the chapter headings that sit
    /// above each section's chunks). Smaller than the old single-chapter
    /// banner — they're navigational markers, not a page title.
    private var sectionTitleSize: CGFloat {
        let base: CGFloat
        switch readingMode {
        case .book:      base = 26
        case .text:      base = 18
        case .technical: base = 15
        }
        return base * scale
    }

    /// Top breathing room above each section title. Applies to ALL
    /// sections including the first — keeping it modest so the kicker
    /// doesn't feel detached from the first chapter heading.
    private var sectionTitleTopPadding: CGFloat {
        switch readingMode {
        case .book:      return 10
        case .text:      return 8
        case .technical: return 6
        }
    }

    private var sectionTitleBottomPadding: CGFloat {
        switch readingMode {
        case .book:      return 10
        case .text:      return 8
        case .technical: return 6
        }
    }

    /// Font for the reading body text, varying by mode and user scale.
    private var chunkFont: Font {
        let base: CGFloat
        switch readingMode {
        case .book:      base = 17
        case .text:      base = 14
        case .technical: base = 12
        }
        let size = base * scale
        switch readingMode {
        case .book:      return .serif(size)
        case .text:      return .sans(size)
        case .technical: return .mono(size)
        }
    }

    /// Additional vertical spacing between lines, per mode + scale.
    private var chunkLineSpacing: CGFloat {
        let base: CGFloat
        switch readingMode {
        case .book:      base = 5
        case .text:      base = 3
        case .technical: base = 4   // mono needs more air or it reads as code
        }
        return base * scale
    }

    /// Top/bottom padding per chunk.
    private var chunkPadding: CGFloat {
        switch readingMode {
        case .book:      return 6
        case .text:      return 2
        case .technical: return 1
        }
    }

    @ViewBuilder
    private func sectionBlock(_ section: SectionDoc, anchoredIds: Set<String>) -> some View {
        // Section title as an inline heading: serif italic, medium size,
        // breathing room above to signal a new chapter without paginating.
        if !section.title.isEmpty {
            Text(section.title)
                .font(.serif(sectionTitleSize, italic: true))
                .kerning(-0.4)
                .foregroundStyle(Tokens.text)
                .padding(.top, sectionTitleTopPadding)
                .padding(.bottom, sectionTitleBottomPadding)
        }
        ForEach(section.chunks) { c in
            chunkParagraph(c, sectionIndex: section.index,
                           hasNote: anchoredIds.contains(c.id))
                .id(c.id)
        }
    }

    @ViewBuilder
    private func chunkParagraph(_ c: ReadingChunk, sectionIndex: Int, hasNote: Bool) -> some View {
        let isHover = hoverId == c.id
        let dimmed  = hoverId != nil && !isHover
        // The chunk the runtime is currently synthesizing / playing.
        // Uses the session's anchor (e.g. "section:2/chunk:5") which
        // matches the chunk's `id` since both are derived from the
        // same `ReadingPosition.anchor()`.
        let isActive = c.id == host.currentSession?.anchor
        HStack(alignment: .top, spacing: 18) {
            // Gutter chunk number.
            if hasNote {
                let noteIdx = (notes.firstIndex(where: { $0.chunkId == c.id }) ?? 0) + 1
                ZStack {
                    Circle()
                        .fill(isHover ? accent.soft : .clear)
                    Circle()
                        .strokeBorder(isHover ? accent.main : Tokens.textGhost, lineWidth: 1)
                    Text("\(noteIdx)")
                        .font(.mono(10))
                        .foregroundStyle(isHover ? accent.main : Tokens.textFaint)
                }
                .frame(width: 24, height: 24)
                .shadow(color: isHover ? accent.glow : .clear, radius: 8)
                .opacity(dimmed ? 0.3 : 1)
            } else {
                Color.clear.frame(width: 24, height: 24)
            }

            Text(c.text)
                .font(chunkFont)
                .foregroundStyle(
                    // Active chunk always reads at full text colour; hover
                    // stays slightly lifted; dimmed for non-hovered when
                    // something else is.
                    isActive ? Tokens.text
                             : isHover ? Tokens.text
                                       : dimmed ? Color(hex: 0xEFE5CF, opacity: 0.25)
                                                : Tokens.textDim
                )
                .lineSpacing(chunkLineSpacing)
                .padding(.vertical, chunkPadding)
                .background(
                    // Active: persistent subtle accent wash; hover: livelier.
                    // Both can combine (mouse over the playing chunk).
                    isActive ? accent.soft.opacity(0.35)
                             : (isHover ? accent.soft.opacity(0.8) : Color.clear)
                )
                .overlay(alignment: .leading) {
                    // Thin accent bar in the gutter for the active chunk —
                    // a tactile "you are here" mark even when the chunk has
                    // no note.
                    if isActive {
                        Rectangle()
                            .fill(accent.main)
                            .frame(width: 2)
                            .padding(.leading, -12)
                    }
                }
                .background(
                    GeometryReader { g in
                        Color.clear.preference(
                            key: ChunkFramesKey.self,
                            value: [ChunkFrame(id: c.id,
                                              rect: g.frame(in: .named("reading-root")))]
                        )
                    }
                )
                .onHover { hovering in
                    if hovering { hoverId = c.id } else if hoverId == c.id { hoverId = nil }
                }
                // Click to seek — snaps playback to this chunk. Uses the
                // section index passed in by the outer ForEach (each
                // section renders its own chunks, so we always know which
                // one `c` belongs to) and the chunk's own `index`
                // (preserved from the runtime via `refreshDocumentView`,
                // not the array position).
                .contentShape(Rectangle())
                .onTapGesture {
                    Task { try? await host.seekToChunk(section: sectionIndex, chunk: c.index) }
                }
        }
    }

    /// Localized "N notes in this chapter" label, picking the right
    /// plural form. Three keys (`_zero`, `_one`, `_other`) keep callers
    /// out of a `.stringsdict` while still reading naturally in both
    /// IT (singular/plural) and EN.
    private func notesCountLabel(_ count: Int) -> String {
        switch count {
        case 0: return T("reading.margin.notes_zero")
        case 1: return T("reading.margin.notes_one")
        default: return String(format: T("reading.margin.notes_other"), count)
        }
    }

    // Margin panel
    private var marginPanel: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 2) {
                    Text(T("reading.margin.kicker"))
                        .font(.mono(10))
                        .tracking(1.5)
                        .foregroundStyle(Tokens.textFaint)
                    Text(notesCountLabel(notes.count))
                        .font(.serif(15, italic: true))
                        .foregroundStyle(Tokens.text)
                }
                Spacer()
                Button(action: openComposeNote) {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(accent.main)
                        .frame(width: 26, height: 26)
                        .background(
                            RoundedRectangle(cornerRadius: 6)
                                .fill(accent.main.opacity(0.08))
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .strokeBorder(accent.main.opacity(0.4), lineWidth: 1)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(T("reading.margin.add.help"))
                .accessibilityLabel(T("reading.margin.add.a11y"))
                .disabled(host.currentSession == nil)
                Text("⌥M")
                    .font(.mono(10))
                    .foregroundStyle(Tokens.textFaint)
                    .padding(.horizontal, 7).padding(.vertical, 3)
                    .overlay(
                        RoundedRectangle(cornerRadius: 4)
                            .strokeBorder(Tokens.line, lineWidth: 1)
                    )
            }
            .padding(.horizontal, 18).padding(.vertical, 14)
            Divider().frame(height: 1).overlay(Tokens.line)
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(Array(notes.enumerated()), id: \.element.id) { idx, note in
                        // `linkedFromChunk` is true when the user is hovering
                        // the chunk that this note anchors to — the note card
                        // reacts by lighting up its index circle even though
                        // the mouse isn't physically over it.
                        let linked = hoverId == note.chunkId
                        if note.live {
                            LiveNoteCard(note: note, idx: idx + 1, accent: accent,
                                         linkedFromChunk: linked,
                                         micLevels: host.micLevels,
                                         onEnter: { hoverId = note.chunkId },
                                         onExit:  { if hoverId == note.chunkId { hoverId = nil } })
                        } else {
                            NoteCard(note: note, idx: idx + 1, accent: accent,
                                     linkedFromChunk: linked,
                                     onEnter: { hoverId = note.chunkId },
                                     onExit:  { if hoverId == note.chunkId { hoverId = nil } },
                                     onDelete: {
                                         if currentPlayingNoteId == note.id {
                                             stopNotePlayback()
                                         }
                                         Task { await host.deleteNote(id: note.id) }
                                     },
                                     onSaveEdit: { newText in
                                         Task { try? await host.updateNote(id: note.id, text: newText) }
                                     },
                                     onPlay: { playNote(note.id) },
                                     onSeek: { seekToNote(note) },
                                     onSeekAndPlay: { seekToNoteAndPlay(note) },
                                     isPlaying: currentPlayingNoteId == note.id,
                                     player: currentPlayingNoteId == note.id ? notePlayer : nil,
                                     levels: currentPlayingNoteId == note.id ? notePlayerLevels : [])
                        }
                    }
                }
            }
            .scrollIndicators(.hidden)
            Divider().frame(height: 1).overlay(Tokens.line)
            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 6).fill(Color.white.opacity(0.04))
                    Circle().fill(accent.main).frame(width: 5, height: 5)
                }
                .frame(width: 24, height: 24)
                rotatingHint
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineLimit(2)
                Spacer(minLength: 8)
                // Settings entrypoint moved to the sidebar footer
                // (always visible, no document required).
            }
            .padding(.horizontal, 18).padding(.vertical, 12)
        }
        .background(Tokens.bg)
    }

    @ViewBuilder
    private var rotatingHint: some View {
        TimelineView(.periodic(from: Date(), by: 7)) { ctx in
            let idx = Int(ctx.date.timeIntervalSinceReferenceDate / 7)
                % marginPanelHintKeys.count
            Text(T(marginPanelHintKeys[abs(idx)]))
                .transition(.opacity)
                .id(idx)
        }
    }

    // Link overlay (curve from chunk → note). When a chunk has multiple
    // notes attached, one curve is drawn per linked note so the user
    // sees the full fan-out (was: only `notes.first` — surprising when
    // a chunk had 2+ notes and only one connector lit up).
    @ViewBuilder
    private var linkOverlay: some View {
        if let hoverId,
           let chunkRect = chunkFrames[hoverId] {
            let linkedNotes = notes.filter { $0.chunkId == hoverId }
            ForEach(linkedNotes, id: \.id) { note in
                if let noteRect = noteFrames[note.id] {
                    chunkToNoteConnector(chunkRect: chunkRect, noteRect: noteRect)
                }
            }
        }
    }

    /// Draws a single bezier from the right edge of `chunkRect` to the
    /// left edge of `noteRect`, with two endpoint dots. Extracted so
    /// `linkOverlay` can call it once per linked note without
    /// duplicating the curve math.
    @ViewBuilder
    private func chunkToNoteConnector(chunkRect: CGRect, noteRect: CGRect) -> some View {
        // Anchor: just inside the bottom-right corner of the hovered
        // chunk. Using (maxY − inset) instead of midY keeps the line's
        // exit point stable and visually "after" the passage, which is
        // what notes are — a mark left at the end of what you read.
        let insetX: CGFloat = 10
        let insetY: CGFloat = 8
        let x1 = chunkRect.maxX - insetX
        let y1 = chunkRect.maxY - insetY
        // Note endpoint: top-left of the note card, a few points down so
        // the line lands on the note's index circle row, not on the
        // border.
        let x2 = noteRect.minX
        let y2 = noteRect.minY + 24

        // Bezier control points: stronger horizontal bias if the two
        // endpoints are close vertically, gentler curve otherwise. This
        // avoids the "wrong chunk" illusion when dy is small.
        let dx = x2 - x1
        let dy = y2 - y1
        let horizontalPull = max(40, abs(dx) * 0.55)
        let c1 = CGPoint(x: x1 + horizontalPull, y: y1 + dy * 0.1)
        let c2 = CGPoint(x: x2 - horizontalPull, y: y2 - dy * 0.1)

        Path { p in
            p.move(to: CGPoint(x: x1, y: y1))
            p.addCurve(to: CGPoint(x: x2, y: y2), control1: c1, control2: c2)
        }
        .stroke(accent.main.opacity(0.85), lineWidth: 1)
        .shadow(color: accent.main, radius: 2)

        Circle().fill(accent.main).frame(width: 6, height: 6).position(x: x1, y: y1)
        Circle().fill(accent.main).frame(width: 6, height: 6).position(x: x2, y: y2)
    }
}

// MARK: — Top toolbar (doc title + center player pill)

struct Toolbar: View {
    var accent: Accent
    var title: String
    var playbackState: PlaybackState
    /// True while the TTS backend is between `SynthesisStarted` and
    /// `SynthesisReady`. Flips the kicker to "SINTETIZZANDO…" with a tiny
    /// spinner so the user knows the ~1 s wait isn't a freeze.
    var synthesizing: Bool = false

    /// Left-side kicker that reflects actual playback state. Synthesis
    /// overrides this (handled separately so the spinner can sit beside
    /// the label). Paused and idle states get their own copy so "IN
    /// ASCOLTO" isn't lingering when nothing is actually being read.
    private var stateKicker: String {
        switch playbackState {
        case .playing:                     return T("reading.toolbar.playing")
        case .paused:                      return T("reading.toolbar.paused")
        case .idle, .finished, .unknown:   return T("reading.toolbar.ready")
        }
    }

    var body: some View {
        // `.firstTextBaseline` anchors the kicker (mono 10) and the doc
        // title (serif 20) on a shared baseline so the two read as one row.
        // Play/pause lives in the sidebar LibRow now — next to the active
        // document entry — so it's visually tied to the thing being read
        // rather than floating at the window corner.
        HStack(alignment: .firstTextBaseline, spacing: 14) {
            if synthesizing {
                HStack(spacing: 6) {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .controlSize(.mini)
                        .tint(accent.main)
                        .alignmentGuide(.firstTextBaseline) { d in d[.bottom] - 2 }
                    Text(T("reading.toolbar.synthesizing"))
                        .font(.mono(10))
                        .tracking(1.5)
                        .foregroundStyle(accent.main)
                        .lineLimit(1)
                        .fixedSize()
                }
            } else {
                Text(stateKicker)
                    .font(.mono(10))
                    .tracking(1.5)
                    .foregroundStyle(playbackState == .playing ? accent.main : Tokens.textFaint)
                    .lineLimit(1)
                    .fixedSize()
            }
            Text(title)
                .font(.serif(16, italic: true))
                .foregroundStyle(Tokens.text)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: 12)
        }
        .padding(.horizontal, 20)
        // Matched to the Sidebar header height so the top edge of the
        // window reads as a single horizontal strip rather than two
        // mismatched bands. 40pt also better fits the actual content
        // (kicker + title) without dead vertical space.
        .frame(height: 40)
    }
}

// MARK: — Live note (active recording) + saved note cards

struct LiveNoteCard: View {
    var note: MarginNote
    var idx: Int
    var accent: Accent
    /// True when the chunk this note anchors to is being hovered in the
    /// reading column. The index circle responds with a subtle pulse so
    /// the reader feels the "marginalia" loop close.
    var linkedFromChunk: Bool = false
    /// Live mic amplitudes from the AEC pipeline. Empty when no AEC is
    /// active — Waveform then falls back to its deterministic sine mock.
    var micLevels: [Float] = []
    var onEnter: () -> Void
    var onExit: () -> Void

    /// "STAI PARLANDO" while the transcript is still empty (dictation in
    /// progress); otherwise echoes the host-supplied status ("SALVATA", …).
    /// Falls back to "NOTA" when status is blank.
    private var kicker: String {
        if note.body.isEmpty { return T("reading.live.speaking") }
        let s = note.status.uppercased()
        return s.isEmpty ? T("reading.live.fallback") : s
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                ZStack {
                    Circle().fill(accent.soft)
                    Circle().strokeBorder(accent.main, lineWidth: 1)
                    Text("\(idx)").font(.mono(10)).foregroundStyle(accent.main)
                }
                .frame(width: 22, height: 22)
                .shadow(color: accent.glow, radius: 5)
                Text(kicker)
                    .font(.mono(9)).tracking(1.5)
                    .foregroundStyle(accent.main)
                Spacer()
                Text(note.duration.isEmpty ? "0:00" : note.duration)
                    .font(.mono(9)).foregroundStyle(Tokens.textDim)
            }
            .padding(.bottom, 10)
            Text(note.body.isEmpty ? "…" : note.body)
                .font(.serif(15, italic: true))
                .foregroundStyle(Tokens.text)
                .lineSpacing(4)
                .padding(.bottom, 12)
            Waveform(count: 56, accent: accent.main, dim: Tokens.textFaint,
                     seed: 0.55, minHeight: 3, maxBump: 16,
                     liveLevels: micLevels)
                .frame(height: 22).padding(.bottom, 4)
        }
        .padding(14)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(LinearGradient(
                    colors: [
                        accent.main.opacity(linkedFromChunk ? 0.42 : 0.28),
                        Tokens.paper.opacity(linkedFromChunk ? 0.55 : 0.4)
                    ],
                    startPoint: .top, endPoint: .bottom))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(accent.main, lineWidth: linkedFromChunk ? 1.5 : 1)
        )
        // Report the NoteFrame rect for the *visible* card (before the
        // outer margin padding), so the link line from the chunk lands
        // exactly on the card's edge, not 14pt to the left of it in the
        // margin gap.
        .background(
            GeometryReader { g in
                Color.clear.preference(
                    key: NoteFramesKey.self,
                    value: [NoteFrame(id: note.id,
                                     rect: g.frame(in: .named("reading-root")))]
                )
            }
        )
        // The LiveNoteCard is already the visually loudest element on
        // screen — when the chunk fires the link, we amplify the whole
        // frame (glow + scale + border) rather than pulsing the circle,
        // so the "marginalia loop" closes around the live note itself.
        .scaleEffect(linkedFromChunk ? 1.015 : 1.0)
        .shadow(color: accent.glow, radius: linkedFromChunk ? 22 : 12)
        .animation(.spring(duration: 0.35, bounce: 0.35),
                   value: linkedFromChunk)
        .padding(.horizontal, 14).padding(.vertical, 12)
        .onHover { hovering in hovering ? onEnter() : onExit() }
    }
}

struct NoteCard: View {
    var note: MarginNote
    var idx: Int
    var accent: Accent
    /// True when the chunk this note anchors to is being hovered in the
    /// reading column. Used to light up and pulse the index circle so the
    /// connection feels "closed" even if the mouse is still on the text.
    var linkedFromChunk: Bool = false
    var onEnter: () -> Void
    var onExit: () -> Void
    /// Optional actions: deleting returns a confirmation to the host,
    /// editing hands the new transcript back. Default no-ops keep the
    /// mock previews compiling unchanged.
    var onDelete: () -> Void = {}
    var onSaveEdit: (String) -> Void = { _ in }
    /// Playback handler. The parent (marginPanel) wires this to a
    /// TTS-preview + rodio play call — when there's no real recorded
    /// audio backing the note (typed or voice-dictation-transcribed),
    /// we synth the body text and play that back. Defaults to a no-op
    /// so preview-only callers don't need to plumb anything.
    var onPlay: () -> Void = {}
    /// Single-click on the card routes here: parent pauses playback
    /// and seeks to the chunk the note is anchored to. The user can
    /// then read silently or hit play.
    var onSeek: () -> Void = {}
    /// Double-click on the card: parent seeks to the chunk AND starts
    /// reading from there immediately. One gesture to "go check this
    /// passage out loud".
    var onSeekAndPlay: () -> Void = {}
    /// True when this note's audio is currently playing — drives the
    /// play/stop glyph swap. Owned by the parent (`ReadingView`) so
    /// the toggle stays in sync with the actual audio engine state.
    var isPlaying: Bool = false
    /// Active AVAudioPlayer when this note is the one currently playing.
    /// Used by the footer waveform to compute `playedFraction` from
    /// `currentTime / duration` so the bar fills left-to-right while
    /// the note plays.
    var player: AVAudioPlayer? = nil
    /// Per-bucket peak amplitudes of the playing note's audio file.
    /// Empty for idle notes — Waveform falls back to its dim sine
    /// (purely decorative) which matches the inactive look.
    var levels: [Float] = []

    @State private var hovered: Bool = false
    @State private var editing: Bool = false
    @State private var editBuffer: String = ""

    /// Treat physical hover and "linked from chunk" uniformly for the
    /// card's accent-coloured elements, but keep the pulse animation
    /// specific to the link-in case.
    private var isActive: Bool { hovered || linkedFromChunk }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 8) {
                ZStack {
                    Circle()
                        .fill(isActive ? accent.soft : .clear)
                    Circle().strokeBorder(isActive ? accent.main : Tokens.textGhost, lineWidth: 1)
                    Text("\(idx)").font(.mono(9)).foregroundStyle(isActive ? accent.main : Tokens.textFaint)
                }
                .frame(width: 20, height: 20)
                // Pulse in only when the signal comes from the chunk side;
                // physical hover over the card stays stable (no jumpiness).
                .scaleEffect(linkedFromChunk ? 1.2 : 1.0)
                .shadow(color: isActive ? accent.glow : .clear,
                        radius: linkedFromChunk ? 10 : (hovered ? 4 : 0))
                .animation(.spring(duration: 0.32, bounce: 0.45),
                           value: linkedFromChunk)
                .animation(.easeOut(duration: 0.15), value: hovered)
                Text(note.when).font(.mono(9)).foregroundStyle(Tokens.textFaint)
                Spacer()
                if !note.status.isEmpty {
                    Text(note.status.uppercased())
                        .font(.mono(8)).tracking(1)
                        .foregroundStyle(accent.main)
                        .padding(.horizontal, 5).padding(.vertical, 2)
                        .background(
                            RoundedRectangle(cornerRadius: 3)
                                .fill(accent.main.opacity(0.08))
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 3)
                                .strokeBorder(accent.main.opacity(0.18), lineWidth: 1)
                        )
                }
                // Edit + Delete buttons — only visible on hover to keep
                // the card clean during regular reading.
                if hovered && !editing {
                    Button(action: {
                        editBuffer = note.body
                        editing = true
                    }) {
                        Image(systemName: "pencil")
                            .font(.system(size: 10))
                            .foregroundStyle(Tokens.textDim)
                            .frame(width: 18, height: 18)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(T("reading.note.edit"))
                    Button(action: onDelete) {
                        Image(systemName: "trash")
                            .font(.system(size: 10))
                            .foregroundStyle(Color.red.opacity(0.75))
                            .frame(width: 18, height: 18)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(T("reading.note.delete"))
                }
            }
            .padding(.bottom, 6)
            Text(note.quote)
                .font(.serif(12, italic: true))
                .foregroundStyle(Tokens.textDim)
                .lineSpacing(3)
                .padding(.leading, 8)
                .overlay(alignment: .leading) {
                    Rectangle().fill(Tokens.line).frame(width: 1)
                }
                .padding(.bottom, 8)
            if editing {
                // Inline editor — TextEditor fills in place of the static
                // body. ⌘↩ saves, Esc reverts. Both also lose focus which
                // SwiftUI handles via `.focused` below.
                VStack(alignment: .trailing, spacing: 6) {
                    TextEditor(text: $editBuffer)
                        .font(.serif(14))
                        .foregroundStyle(Tokens.text)
                        .scrollContentBackground(.hidden)
                        .frame(minHeight: 60)
                        .padding(6)
                        .background(
                            RoundedRectangle(cornerRadius: 6)
                                .fill(Color.white.opacity(0.03))
                        )
                        .overlay(
                            RoundedRectangle(cornerRadius: 6)
                                .strokeBorder(accent.main.opacity(0.3), lineWidth: 1)
                        )
                    HStack(spacing: 8) {
                        Button(T("common.cancel")) { editing = false }
                            .buttonStyle(.plain)
                            .font(.mono(10))
                            .foregroundStyle(Tokens.textFaint)
                        Button(T("common.save")) {
                            onSaveEdit(editBuffer.trimmingCharacters(in: .whitespacesAndNewlines))
                            editing = false
                        }
                        .buttonStyle(.plain)
                        .font(.mono(10))
                        .foregroundStyle(accent.main)
                        .disabled(editBuffer.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
                .padding(.bottom, 10)
            } else {
                Text(note.body)
                    .font(.serif(14))
                    .foregroundStyle(Tokens.text)
                    .lineSpacing(4)
                    .padding(.bottom, 10)
            }
            HStack(spacing: 10) {
                // Play/Stop toggle — the parent owns the playback
                // engine and `isPlaying` reflects whether THIS note's
                // audio is currently coming out of the speakers. Tap
                // routes through `onPlay`, which the parent treats as
                // a toggle (start when idle, stop when this note is
                // active, switch over when a different note is active).
                Button(action: onPlay) {
                    ZStack {
                        Circle()
                            .fill(isPlaying ? accent.main : Color.clear)
                        Circle()
                            .strokeBorder(isPlaying ? accent.main : Tokens.textGhost, lineWidth: 1)
                        if isPlaying {
                            Rectangle()
                                .fill(Tokens.bg)
                                .frame(width: 7, height: 7)
                        } else {
                            Image(systemName: "play.fill")
                                .font(.system(size: 8))
                                .foregroundStyle(Tokens.textDim)
                                .offset(x: 1)
                        }
                    }
                    .frame(width: 22, height: 22)
                    // Hit-test on the full 22pt square instead of just
                    // the play triangle / stop square — the user
                    // shouldn't have to land on the icon pixels.
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(isPlaying ? T("reading.note.stop") : T("reading.note.play"))
                .accessibilityLabel(isPlaying ? T("reading.note.stop") : T("reading.note.play"))
                if !note.duration.isEmpty {
                    Text(note.duration)
                        .font(.mono(9))
                        .foregroundStyle(Tokens.textFaint)
                }
                // Scrolling waveform — mirrors the SettingsView preview
                // pattern. While `isPlaying`, polls the AVAudioPlayer's
                // currentTime/duration at ~20Hz and feeds the ratio in
                // as `playedFraction`, so the bar visibly fills left-to-
                // right. When idle, the fraction is 0 and the bar sits
                // dimmed in place.
                TimelineView(.periodic(from: Date(), by: 0.05)) { _ in
                    let fraction: Double = {
                        guard isPlaying,
                              let p = player,
                              p.duration > 0
                        else { return 0 }
                        return min(1.0, p.currentTime / p.duration)
                    }()
                    Waveform(
                        count: 32,
                        playedFraction: fraction,
                        accent: accent.main,
                        dim: accent.main.opacity(0.3),
                        seed: 0.42, minHeight: 2, maxBump: 12,
                        liveLevels: levels.isEmpty ? nil : levels
                    )
                    .frame(maxWidth: .infinity)
                    .frame(height: 18)
                }
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(isActive ? Color(hex: 0xEFE5CF, opacity: 0.02) : Color.clear)
        .overlay(alignment: .leading) {
            Rectangle().fill(isActive ? accent.main : .clear).frame(width: 2)
        }
        .overlay(alignment: .bottom) {
            Rectangle().fill(Tokens.lineSoft).frame(height: 1)
        }
        .contentShape(Rectangle())
        // Single-click → seek only. Double-click → seek and start
        // reading from there. The double-tap modifier MUST be added
        // BEFORE the single-tap one so SwiftUI's gesture resolver
        // gives the multi-tap priority and waits the recognition
        // window before firing the single-tap closure.
        .onTapGesture(count: 2) { onSeekAndPlay() }
        .onTapGesture(count: 1) { onSeek() }
        .animation(.easeOut(duration: 0.18), value: isActive)
        .onHover { h in
            hovered = h
            h ? onEnter() : onExit()
        }
        .background(
            GeometryReader { g in
                Color.clear.preference(
                    key: NoteFramesKey.self,
                    value: [NoteFrame(id: note.id,
                                     rect: g.frame(in: .named("reading-root")))]
                )
            }
        )
    }
}

// MARK: — Note compose sheet

/// Modal that lets the user add a note to the current chunk. Speech-to-text
/// is always-on while the sheet is visible: opening the sheet calls
/// `host.startDictation()` and the transcript — when it arrives via the
/// `voiceNoteTranscribed` event — gets copied into the text field. The
/// user can still type directly, or edit whatever the STT produced.
///
/// Save semantics:
/// - If the dictation produced a note (we capture its id from the
///   runtime) → `updateNote(id:text:)` so the final edited version wins.
/// - Otherwise (user typed or dictation yielded nothing) → `createNote`.
///
/// Cancel: if there's an auto-created note from dictation, we delete it
/// so the sheet leaves no trace on the library.
struct NoteComposeSheet<Host: MarginaliaHost>: View {
    var accent: Accent
    @ObservedObject var host: Host
    @Binding var text: String
    var anchorLabel: String
    var onClose: () -> Void

    @State private var dictatedNoteId: String? = nil
    @State private var userEdited: Bool = false
    @FocusState private var fieldFocused: Bool

    private var canSave: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private var listeningActive: Bool {
        // Still in recording phase if the live note exists but hasn't
        // accumulated a transcript yet (body empty + live flag).
        guard let live = host.liveNote else { return false }
        return live.live && live.body.isEmpty
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 8) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(T("reading.compose.kicker"))
                        .font(.mono(10)).tracking(1.5)
                        .foregroundStyle(Tokens.textFaint)
                    if !anchorLabel.isEmpty {
                        Text(anchorLabel)
                            .font(.serif(14, italic: true))
                            .foregroundStyle(Tokens.textDim)
                    }
                }
                Spacer()
                listeningBadge
            }

            TextEditor(text: $text)
                .font(.serif(15))
                .foregroundStyle(Tokens.text)
                .scrollContentBackground(.hidden)
                .frame(minHeight: 140)
                .padding(8)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.white.opacity(0.03))
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .strokeBorder(accent.main.opacity(0.3), lineWidth: 1)
                )
                .focused($fieldFocused)
                .onChange(of: text) { _, newValue in
                    // Ignore self-applied partial updates: when the
                    // dictation partial arrives and we copy it into
                    // `text`, this `.onChange` fires too. Without this
                    // guard, our own write would flip `userEdited` on
                    // and freeze subsequent partials at the first word.
                    if newValue == host.liveNote?.body { return }
                    userEdited = true
                }

            HStack(spacing: 10) {
                Text(T("reading.compose.hint"))
                    .font(.serif(12, italic: true))
                    .foregroundStyle(Tokens.textFaint)

                Spacer()

                Button(T("common.cancel")) { cancel() }
                    .buttonStyle(.plain)
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .keyboardShortcut(.cancelAction)

                Button(action: save) {
                    Text(T("common.save"))
                        .font(.sans(13, weight: .medium))
                        .foregroundStyle(canSave ? Tokens.bg : Tokens.textFaint)
                        .padding(.horizontal, 18).padding(.vertical, 7)
                        .background(Capsule().fill(canSave ? accent.main : Color.white.opacity(0.05)))
                }
                .buttonStyle(.plain)
                .keyboardShortcut(.defaultAction)
                .disabled(!canSave)
            }
        }
        .padding(20)
        .frame(width: 460)
        .background(Tokens.bg)
        .onAppear {
            fieldFocused = true
            text = ""
            dictatedNoteId = nil
            userEdited = false
            // Fire dictation in the background — the user doesn't need
            // to click anything. If they prefer to type, the STT silence
            // timeout ends the capture cleanly.
            host.startDictation()
        }
        .onChange(of: host.liveNote?.body) { _, newBody in
            // A live note arriving with non-empty body means the STT
            // produced a transcript. Copy it into the draft (but don't
            // clobber what the user has been typing in the meantime).
            guard let newBody, !newBody.isEmpty else { return }
            if !userEdited { text = newBody }
            if let id = host.liveNote?.id, id != "live" {
                dictatedNoteId = id
            }
        }
    }

    @ViewBuilder
    private var listeningBadge: some View {
        if listeningActive {
            HStack(spacing: 6) {
                Circle()
                    .fill(accent.main)
                    .frame(width: 6, height: 6)
                    .shadow(color: accent.main, radius: 4)
                Text(T("reading.compose.listening"))
                    .font(.mono(9)).tracking(1.5)
                    .foregroundStyle(accent.main)
            }
        } else if dictatedNoteId != nil {
            Text(T("reading.compose.transcribed"))
                .font(.mono(9)).tracking(1.5)
                .foregroundStyle(Tokens.textDim)
        }
    }

    private func save() {
        let final = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !final.isEmpty else { return }
        let id = dictatedNoteId
        onClose()
        host.clearLiveNote()
        // Suppress any dictation that's still in flight (e.g. user opened
        // the sheet, started speaking, then changed their mind and typed
        // text instead). Without this, the late-arriving transcript
        // would create a SECOND, phantom note alongside the typed one —
        // and on a silent dictation it'd land 60s later as the timeout
        // error string. Same flag the cancel() path uses.
        if id == nil {
            host.cancelPendingDictation()
        }
        Task {
            if let id {
                try? await host.updateNote(id: id, text: final)
            } else {
                try? await host.createNote(text: final)
            }
        }
    }

    private func cancel() {
        let id = dictatedNoteId
        onClose()
        // Always flag a cancel — covers two cases:
        // 1. Dictation already produced a transcript (id captured) →
        //    deleteNote does the cleanup directly below.
        // 2. Dictation still in flight (no id yet) → the next
        //    `voiceNoteTranscribed` will see the flag and delete the
        //    note the runtime is about to persist.
        host.cancelPendingDictation()
        if let id {
            Task { await host.deleteNote(id: id) }
        }
    }
}
