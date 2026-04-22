import SwiftUI

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

// MARK: — PreferenceKeys for link-overlay geometry

struct ChunkFrame: Equatable { let id: String; let rect: CGRect }
struct NoteFrame: Equatable { let id: String; let rect: CGRect }

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
    public var onOpenSettings: () -> Void

    @State private var hoverId: String? = nil
    @State private var chunkFrames: [String: CGRect] = [:]
    @State private var noteFrames: [String: CGRect] = [:]
    @State private var containerWidth: CGFloat = 0
    /// Persisted across launches. AppStorage with a RawRepresentable enum
    /// requires a non-optional default — ReadingMode.book is used when the
    /// stored value is absent or unparseable.
    @AppStorage(ReadingPrefs.modeKey) private var readingMode: ReadingMode = .book
    /// Per-user text-size multiplier, persisted. 1.0 = design default.
    @AppStorage(ReadingPrefs.scaleKey) private var fontScale: Double = 1.0

    public init(accent: Accent, host: Host,
                onOpenSettings: @escaping () -> Void = {}) {
        self.accent = accent
        self._host = ObservedObject(initialValue: host)
        self.onOpenSettings = onOpenSettings
    }

    /// Chunks of the currently-visible section. Expects a live session —
    /// `MarginaliaWindow` gates `.reading` mode on `currentSession != nil`,
    /// so by the time we get here the host has a document to render. If
    /// somehow it doesn't (e.g. the view is mounted while the runtime is
    /// still initialising), we return an empty list and the reading column
    /// renders just its header and gradient wash — better than leaking
    /// mock Thomas Mann text.
    private var chunks: [ReadingChunk] {
        guard let section = currentSection else { return [] }
        return section.chunks
    }

    private var currentSection: SectionDoc? {
        guard let doc = host.currentDocument else { return nil }
        return doc.sections.first(where: { $0.index == (host.currentSession?.sectionIndex ?? -1) })
            ?? doc.sections.first
    }

    private var notes: [MarginNote] {
        var all = host.notes
        if let live = host.liveNote { all.insert(live, at: 0) }
        return all
    }

    public var body: some View {
        VStack(spacing: 0) {
            Toolbar(
                accent: accent,
                title: host.currentSession?.documentTitle ?? "",
                subtitle: sessionSubtitle,
                playbackState: host.currentSession?.playbackState ?? .idle,
                synthesizing: host.synthesizingAnchor != nil,
                onTogglePlay: togglePlay
            )
            Divider().frame(height: 1).overlay(Tokens.line)
            mainArea
        }
        .background(Tokens.bg)
    }

    private var sessionSubtitle: String {
        guard let s = host.currentSession else { return "" }
        return "capitolo \(s.sectionIndex + 1) · chunk \(s.chunkIndex + 1)"
    }

    /// Uppercased document title — the kicker line above the big title.
    /// Dropped to an empty string if we somehow render without a document
    /// (MarginaliaWindow gates this, but defense-in-depth).
    private var headerKicker: String {
        (host.currentDocument?.title ?? "").uppercased()
    }

    /// "Capitolo N" — 1-based chapter label derived from the section index.
    private var headerChapterLabel: String {
        guard let idx = host.currentSession?.sectionIndex else { return "" }
        return "Capitolo \(idx + 1)"
    }

    /// Section title in italics below the chapter label. Empty strings
    /// suppress the second line entirely (see the `!isEmpty` guard upstream).
    private var headerSectionTitle: String {
        host.currentSession?.sectionTitle ?? currentSection?.title ?? ""
    }

    private func togglePlay() {
        Task {
            if host.currentSession?.playbackState == .playing {
                try? await host.pause()
            } else {
                try? await host.resume()
            }
        }
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
                        // Header / title — live data from the current
                        // session. Size drops in text mode and respects the
                        // user's fontScale setting.
                        VStack(alignment: .leading, spacing: 12) {
                            Text(headerKicker)
                                .font(.mono(10))
                                .tracking(2)
                                .foregroundStyle(Tokens.textFaint)
                            VStack(alignment: .leading, spacing: 0) {
                                Text(headerChapterLabel)
                                    .font(.serif(titleSize, weight: .medium))
                                    .kerning(-0.6)
                                    .foregroundStyle(Tokens.text)
                                if !headerSectionTitle.isEmpty {
                                    Text(headerSectionTitle)
                                        .font(.serif(titleSize, italic: true))
                                        .kerning(-0.6)
                                        .foregroundStyle(Tokens.textDim)
                                }
                            }
                        }
                        .padding(.bottom, headerPaddingBottom)

                        // Chunks. No inter-chunk spacing — flows as continuous
                        // prose. The gutter number + active-highlight still
                        // signal chunk boundaries without visual "stacchi".
                        VStack(alignment: .leading, spacing: 0) {
                            ForEach(chunks) { c in
                                chunkParagraph(c, hasNote: anchoredIds.contains(c.id))
                                    .id(c.id)
                            }
                        }
                        .frame(maxWidth: 640, alignment: .leading)
                    }
                    .padding(.horizontal, 88)
                    .padding(.top, 54)
                    .padding(.bottom, 60)
                }
                .scrollIndicators(.hidden)
                // Follow auto-advance: centre the chunk the runtime is
                // currently reading. `anchor` in SessionSnapshot maps 1:1 to
                // the chunk id in the document view, so a straight
                // `scrollTo` works.
                .onChange(of: host.currentSession?.anchor) { _, newAnchor in
                    guard let anchor = newAnchor else { return }
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
            .accessibilityLabel("Aumenta testo")

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
            .accessibilityLabel("Rimpicciolisci testo")
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
            modeButton(.book,      icon: "book",                             label: "libro")
            modeButton(.text,      icon: "text.alignleft",                   label: "testo")
            modeButton(.technical, icon: "chevron.left.forwardslash.chevron.right",
                                    label: "tecnico")
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
        }
        .buttonStyle(.plain)
        .help("Modalità \(label)")
        .accessibilityLabel("Modalità \(label)")
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    /// Multiplier the fontScale slider drives, clamped to safety.
    private var scale: CGFloat {
        CGFloat(max(ReadingPrefs.scaleMin, min(ReadingPrefs.scaleMax, fontScale)))
    }

    private var headerPaddingBottom: CGFloat {
        switch readingMode {
        case .book:      return 44
        case .text:      return 28
        case .technical: return 20
        }
    }

    /// Font size for the chapter title (e.g. "Capitolo III"). Drops
    /// significantly in text/technical modes since they're scanning views,
    /// not settling-in views.
    private var titleSize: CGFloat {
        let base: CGFloat
        switch readingMode {
        case .book:      base = 46
        case .text:      base = 26
        case .technical: base = 20
        }
        return base * scale
    }

    /// Font for the reading body text, varying by mode and user scale.
    private var chunkFont: Font {
        let base: CGFloat
        switch readingMode {
        case .book:      base = 21
        case .text:      base = 15
        case .technical: base = 13
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
        case .book:      base = 6
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
    private func chunkParagraph(_ c: ReadingChunk, hasNote: Bool) -> some View {
        let isHover = hoverId == c.id
        let dimmed  = hoverId != nil && !isHover
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
                    isHover ? Tokens.text
                             : dimmed ? Color(hex: 0xEFE5CF, opacity: 0.25)
                                      : Tokens.textDim
                )
                .lineSpacing(chunkLineSpacing)
                .padding(.vertical, chunkPadding)
                .background(isHover ? accent.soft.opacity(0.8) : Color.clear)
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
                // section from the currently-visible SectionDoc and the
                // chunk's own `index` (preserved from the runtime via
                // `refreshDocumentView`, not the array position).
                .contentShape(Rectangle())
                .onTapGesture {
                    guard let sec = currentSection?.index else { return }
                    Task { try? await host.seekToChunk(section: sec, chunk: c.index) }
                }
        }
    }

    // Margin panel
    private var marginPanel: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("MARGINALIA")
                        .font(.mono(10))
                        .tracking(1.5)
                        .foregroundStyle(Tokens.textFaint)
                    Text("\(notes.count) note in questo capitolo")
                        .font(.serif(15, italic: true))
                        .foregroundStyle(Tokens.text)
                }
                Spacer()
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
                                         onEnter: { hoverId = note.chunkId },
                                         onExit:  { if hoverId == note.chunkId { hoverId = nil } })
                        } else {
                            NoteCard(note: note, idx: idx + 1, accent: accent,
                                     linkedFromChunk: linked,
                                     onEnter: { hoverId = note.chunkId },
                                     onExit:  { if hoverId == note.chunkId { hoverId = nil } })
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
                Text("passa sopra una nota per vedere il chunk collegato")
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineLimit(2)
                Spacer(minLength: 8)
                // Gear lives here now (moved from the sidebar header), so the
                // settings entrypoint sits in a region not already occupied
                // by the search/ingest controls on the left column.
                Button(action: onOpenSettings) {
                    Image(systemName: "gearshape")
                        .font(.system(size: 11, weight: .regular))
                        .foregroundStyle(Tokens.textDim)
                        .frame(width: 28, height: 28)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .strokeBorder(Tokens.textGhost, lineWidth: 1)
                        )
                }
                .buttonStyle(.plain)
                .help("Impostazioni")
                .accessibilityLabel("Apri impostazioni")
                .keyboardShortcut(",", modifiers: [.command])
            }
            .padding(.horizontal, 18).padding(.vertical, 12)
        }
        .background(Tokens.bg)
    }

    // Link overlay (curve from chunk → note)
    @ViewBuilder
    private var linkOverlay: some View {
        if let hoverId,
           let note = notes.first(where: { $0.chunkId == hoverId }),
           let chunkRect = chunkFrames[hoverId],
           let noteRect = noteFrames[note.id] {
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
}

// MARK: — Top toolbar (doc title + center player pill)

struct Toolbar: View {
    var accent: Accent
    var title: String
    var subtitle: String
    var playbackState: PlaybackState
    /// True while the TTS backend is between `SynthesisStarted` and
    /// `SynthesisReady`. Flips the kicker to "SINTETIZZANDO…" with a tiny
    /// spinner so the user knows the ~1 s wait isn't a freeze.
    var synthesizing: Bool = false
    var onTogglePlay: () -> Void

    var body: some View {
        // `.firstTextBaseline` anchors IN ASCOLTO (mono 10), "La montagna
        // incantata" (serif 17) and the subtitle (serif 14) on a shared
        // baseline so the eye reads them as one row. Non-text items (Circle,
        // playerControls) centre themselves to that baseline too.
        HStack(alignment: .firstTextBaseline, spacing: 14) {
            if synthesizing {
                HStack(spacing: 6) {
                    ProgressView()
                        .progressViewStyle(.circular)
                        .controlSize(.mini)
                        .tint(accent.main)
                        .alignmentGuide(.firstTextBaseline) { d in d[.bottom] - 2 }
                    Text("SINTETIZZANDO")
                        .font(.mono(10))
                        .tracking(1.5)
                        .foregroundStyle(accent.main)
                        .lineLimit(1)
                        .fixedSize()
                }
            } else {
                Text("IN ASCOLTO")
                    .font(.mono(10))
                    .tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                    .lineLimit(1)
                    .fixedSize()
            }
            // Same size as "Impostazioni" in the Settings top-bar so the
            // two surfaces feel like a matching pair of headers.
            Text(title)
                .font(.serif(20, italic: true))
                .foregroundStyle(Tokens.text)
                .lineLimit(1)
                .truncationMode(.tail)
            Circle()
                .fill(Tokens.textFaint)
                .frame(width: 3, height: 3)
                .alignmentGuide(.firstTextBaseline) { _ in 0 }  // sit on baseline
            Text(subtitle)
                .font(.serif(14))
                .foregroundStyle(Tokens.textDim)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: 12)

            // Right: inline player controls — no pill, no background.
            // `fixedSize` + `layoutPriority` pins them at their intrinsic
            // width so 04:23 / 12:03 / 1.0× never wrap onto two lines.
            playerControls(accent: accent)
                .fixedSize(horizontal: true, vertical: false)
                .layoutPriority(2)
        }
        .padding(.horizontal, 20)
        .frame(height: 52)
    }

    @ViewBuilder
    private func playerControls(accent: Accent) -> some View {
        HStack(spacing: 10) {
            Text("04:23")
                .font(.mono(10))
                .foregroundStyle(Tokens.textFaint)
                .lineLimit(1)
                .fixedSize()
                .monospacedDigit()
            Waveform(count: 44, playedFraction: 0.34, accent: accent.main, dim: Tokens.textDim,
                     seed: 0.55, minHeight: 3, maxBump: 11)
                .frame(width: 120, height: 18)
            Text("12:03")
                .font(.mono(10))
                .foregroundStyle(Tokens.textFaint)
                .lineLimit(1)
                .fixedSize()
                .monospacedDigit()

            // Play / Pause toggle on accent circle.
            Button(action: onTogglePlay) {
                ZStack {
                    Circle()
                        .fill(accent.main)
                        .shadow(color: accent.glow, radius: 8)
                    if playbackState == .playing {
                        HStack(spacing: 2.5) {
                            Rectangle().fill(Tokens.bg).frame(width: 2.5, height: 10)
                            Rectangle().fill(Tokens.bg).frame(width: 2.5, height: 10)
                        }
                    } else {
                        Path { p in
                            p.move(to: CGPoint(x: 10, y: 8))
                            p.addLine(to: CGPoint(x: 10, y: 20))
                            p.addLine(to: CGPoint(x: 20, y: 14))
                            p.closeSubpath()
                        }
                        .fill(Tokens.bg)
                        .frame(width: 28, height: 28)
                    }
                }
                .frame(width: 28, height: 28)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(playbackState == .playing ? "Pausa" : "Riprendi lettura")
            .keyboardShortcut(.space, modifiers: [])

            Text("1.0×")
                .font(.mono(11))
                .foregroundStyle(Tokens.textDim)
                .lineLimit(1)
                .fixedSize()
        }
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
    var onEnter: () -> Void
    var onExit: () -> Void

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
                Text("STAI PARLANDO")
                    .font(.mono(9)).tracking(1.5)
                    .foregroundStyle(accent.main)
                Spacer()
                Text("0:14").font(.mono(9)).foregroundStyle(Tokens.textDim)
            }
            .padding(.bottom, 10)
            (Text("\u{201C}Qui il tempo è trattato come uno spazio che si può ")
             + Text("attraversare").foregroundColor(accent.main))
                .font(.serif(15, italic: true))
                .foregroundStyle(Tokens.text)
                .lineSpacing(4)
                .padding(.bottom, 12)
            Waveform(count: 56, accent: accent.main, dim: Tokens.textFaint,
                     seed: 0.55, minHeight: 3, maxBump: 16)
                .frame(height: 22).padding(.bottom, 4)
            (Text("di' ") + Text("\u{201C}fatto\u{201D}").foregroundColor(Tokens.textDim)
             + Text(" per salvare · ") + Text("\u{201C}rielabora\u{201D}").foregroundColor(Tokens.textDim)
             + Text(" per riscrivere"))
                .font(.serif(12, italic: true))
                .foregroundStyle(Tokens.textFaint)
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

    @State private var hovered: Bool = false

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
            Text(note.body)
                .font(.serif(14))
                .foregroundStyle(Tokens.text)
                .lineSpacing(4)
                .padding(.bottom, 10)
            HStack(spacing: 10) {
                ZStack {
                    Circle().strokeBorder(Tokens.textGhost, lineWidth: 1)
                    Path { p in
                        p.move(to: CGPoint(x: 7, y: 5))
                        p.addLine(to: CGPoint(x: 13, y: 10))
                        p.addLine(to: CGPoint(x: 7, y: 15))
                        p.closeSubpath()
                    }
                    .fill(Tokens.textDim)
                }
                .frame(width: 20, height: 20)
                Waveform(count: 40, accent: Tokens.textDim, dim: Tokens.textDim,
                         seed: 0.5, minHeight: 2, maxBump: 7)
                    .frame(height: 10)
                Text(note.duration).font(.mono(9)).foregroundStyle(Tokens.textFaint)
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
