import SwiftUI

/// Left-hand library sidebar (264 px wide). Uses the injected `library`
/// slice (either the mock's static list or the real FFI host's @Published
/// `library`) so the component stays pure — no host binding here.
public struct Sidebar: View {
    public var accent: Accent
    public var library: [LibraryEntry]
    public var micLevels: [Float]
    public var ttsLevels: [Float]
    /// Display name of the active TTS voice (e.g. "Sara"). Shown in the
    /// footer so the user always knows which voice is speaking.
    public var voiceName: String
    /// Short language code (e.g. "IT"). Empty hides the subtitle line.
    public var languageCode: String
    /// Playback state of the currently-active document — drives the
    /// inline play/pause button on the active library row. `.idle` hides
    /// the button so inactive rows stay visually quiet.
    public var playbackState: PlaybackState
    public var onTogglePlay: () -> Void
    public var onAddDocument: () -> Void
    public var onOpenDocument: (String) -> Void
    /// Optional: triggered by the sidebar row's context menu "Rimuovi".
    /// Default no-op so preview / mock don't need to wire it.
    public var onDeleteDocument: (String) -> Void = { _ in }
    /// Triggered by the row's "Apri file in editor…" context-menu entry.
    /// The host opens the source file via `NSWorkspace.shared.open`.
    public var onOpenSourceInEditor: (String) -> Void = { _ in }
    /// Triggered by the row's "Ricarica da disco" context-menu entry.
    /// Confirmation + actual reload happen in the caller
    /// (`MarginaliaWindow`); this layer only fires the intent.
    public var onReloadDocument: (String) -> Void = { _ in }
    /// Open the Settings page. Lives in the sidebar footer (next to
    /// the voice meters) so the user can reach Settings from any
    /// context — including when no document is open and the right
    /// margin panel isn't rendered.
    public var onOpenSettings: () -> Void = {}

    /// Free-text filter for the library list. Case-insensitive, matches on
    /// title and any subtitle text. Bound to the sidebar's search field;
    /// also focusable via ⌘K (see `.onReceive` on MarginaliaWindow — TBD).
    @State private var libraryFilter: String = ""
    /// Explicit focus management for the search TextField so it doesn't
    /// auto-grab keyboard focus on window activation (which was
    /// swallowing shortcuts like Space for play/pause). Only taps on the
    /// field set it to true; Esc clears it.
    @FocusState private var searchFocused: Bool

    public init(accent: Accent,
                library: [LibraryEntry],
                micLevels: [Float] = [],
                ttsLevels: [Float] = [],
                voiceName: String = "",
                languageCode: String = "",
                playbackState: PlaybackState = .idle,
                onTogglePlay: @escaping () -> Void = {},
                onAddDocument: @escaping () -> Void = {},
                onOpenDocument: @escaping (String) -> Void = { _ in },
                onDeleteDocument: @escaping (String) -> Void = { _ in },
                onOpenSourceInEditor: @escaping (String) -> Void = { _ in },
                onReloadDocument: @escaping (String) -> Void = { _ in },
                onOpenSettings: @escaping () -> Void = {}) {
        self.accent = accent
        self.library = library
        self.micLevels = micLevels
        self.ttsLevels = ttsLevels
        self.voiceName = voiceName
        self.languageCode = languageCode
        self.playbackState = playbackState
        self.onTogglePlay = onTogglePlay
        self.onAddDocument = onAddDocument
        self.onOpenDocument = onOpenDocument
        self.onDeleteDocument = onDeleteDocument
        self.onOpenSourceInEditor = onOpenSourceInEditor
        self.onReloadDocument = onReloadDocument
        self.onOpenSettings = onOpenSettings
    }

    /// Filtered library — if `libraryFilter` is empty, return everything.
    /// Applies lowercased "contains" to the title + subtitle (whatever the
    /// entry chooses to expose). Result stays sorted as the host supplied
    /// (typically by last-opened desc).
    private var filteredLibrary: [LibraryEntry] {
        let q = libraryFilter.trimmingCharacters(in: .whitespaces).lowercased()
        if q.isEmpty { return library }
        return library.filter { entry in
            entry.title.lowercased().contains(q)
                || entry.subtitle.lowercased().contains(q)
        }
    }

    public var body: some View {
        VStack(spacing: 0) {
            // Invisible focusable absorber: macOS's first-responder
            // chain hands keyboard focus to the first focusable view in
            // the window on activation. Without this, the search
            // TextField below gets it, swallowing every key press
            // (Space for play/pause, Option-N for new note, …) until
            // the user clicks elsewhere. A 0×0 focusable Color absorbs
            // that initial focus harmlessly.
            Color.clear
                .frame(width: 0, height: 0)
                .focusable(true)
            header
            Divider().frame(height: 1).overlay(Tokens.line)
            searchBarRow
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    SideSection(title: T("sidebar.library.title"), accent: accent) {
                        ForEach(filteredLibrary) { entry in
                            Button(action: { onOpenDocument(entry.id) }) {
                                LibRow(entry: entry, accent: accent)
                            }
                            .buttonStyle(.plain)
                            .contextMenu {
                                Button(T("sidebar.library.open")) { onOpenDocument(entry.id) }
                                Button(T("sidebar.library.edit")) {
                                    onOpenSourceInEditor(entry.id)
                                }
                                .disabled(entry.sourcePath.isEmpty)
                                Button(T("sidebar.library.reload")) {
                                    onReloadDocument(entry.id)
                                }
                                .disabled(!entry.needsReload)
                                Divider()
                                // Confirmation is handled by the caller
                                // (`MarginaliaWindow`) — at this layer we
                                // just fire the intent.
                                Button(role: .destructive) {
                                    onDeleteDocument(entry.id)
                                } label: {
                                    Text(T("sidebar.library.remove"))
                                }
                            }
                        }
                        if library.isEmpty {
                            Text(T("sidebar.library.empty"))
                                .font(.serif(12, italic: true))
                                .foregroundStyle(Tokens.textFaint)
                                .padding(.horizontal, 18).padding(.vertical, 10)
                        } else if filteredLibrary.isEmpty {
                            // Search produced nothing — tell the user why
                            // there's silence instead of showing a blank list.
                            Text(String(format: T("sidebar.library.no_results"), libraryFilter))
                                .font(.serif(12, italic: true))
                                .foregroundStyle(Tokens.textFaint)
                                .padding(.horizontal, 18).padding(.vertical, 10)
                        }
                    }
                }
            }
            .scrollIndicators(.hidden)
            Divider().frame(height: 1).overlay(Tokens.line)
            footer
        }
        .frame(width: 264)
        .background(Tokens.bg2)
    }

    /// Header: wordmark only. Gear lives in the footer (always
    /// visible), "+" is next to the search bar. Compact 40pt to
    /// match the trimmed Toolbar in `ReadingView` so the top edge
    /// reads as a single horizontal band.
    private var header: some View {
        HStack {
            Text("Marginalia")
                .font(.serif(16, italic: true))
                .foregroundStyle(Tokens.text)
            Spacer()
        }
        // `.hiddenTitleBar` leaves the native traffic-light controls
        // floating at x≈20…80. The 82-pt leading padding clears them.
        .padding(.leading, 82)
        .padding(.trailing, 18)
        .frame(height: 40)
    }

    /// Search input + "+" import button sit in the same row.
    private var searchBarRow: some View {
        HStack(spacing: 8) {
            searchBar
            Button(action: onAddDocument) {
                Image(systemName: "plus")
                    .font(.system(size: 11, weight: .regular))
                    .foregroundStyle(Tokens.textDim)
                    .frame(width: 28, height: 28)
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .strokeBorder(Tokens.textGhost, lineWidth: 1)
                    )
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help(T("sidebar.import.help"))
            .accessibilityLabel(T("sidebar.import.a11y"))
        }
        .padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 10)
    }

    /// Search input, used inside `searchBarRow` which also hosts the "+".
    /// Real `TextField` bound to `libraryFilter`; `filteredLibrary` reads
    /// it. `.textFieldStyle(.plain)` removes AppKit's default chrome so
    /// the rounded container below is the only visible frame.
    private var searchBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 10))
                .foregroundStyle(Tokens.textFaint)
            TextField(T("sidebar.search.placeholder"), text: $libraryFilter)
                .textFieldStyle(.plain)
                .font(.sans(12))
                .foregroundStyle(Tokens.text)
                .focused($searchFocused)
                // Esc while typing → unfocus + clear so the play/pause
                // spacebar shortcut and other key commands start firing
                // again without clicking elsewhere.
                .onKeyPress(.escape) {
                    libraryFilter = ""
                    searchFocused = false
                    return .handled
                }
            if !libraryFilter.isEmpty {
                Button(action: {
                    libraryFilter = ""
                    searchFocused = false
                }) {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 10))
                        .foregroundStyle(Tokens.textFaint)
                        .frame(width: 18, height: 18)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(T("sidebar.search.clear"))
            }
        }
        .padding(.horizontal, 10).padding(.vertical, 7)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.white.opacity(0.03))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }

    private var footer: some View {
        VStack(spacing: 8) {
            HStack(spacing: 10) {
                footerPlayButton
                VStack(alignment: .leading, spacing: 1) {
                    Text(voiceName.isEmpty
                         ? T("sidebar.footer.voice_empty")
                         : String(format: T("sidebar.footer.voice"), voiceName))
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.text)
                    if !languageCode.isEmpty {
                        Text(languageCode)
                            .font(.mono(9))
                            .foregroundStyle(Tokens.textFaint)
                    }
                }
                Spacer()
                // Settings entrypoint that's always visible — the
                // margin panel only renders when a document is open,
                // and the toolbar is now minimal, so the sidebar
                // footer is the one place every user state can reach.
                Button(action: onOpenSettings) {
                    Image(systemName: "gearshape")
                        .font(.system(size: 12, weight: .regular))
                        .foregroundStyle(Tokens.textDim)
                        .frame(width: 28, height: 28)
                        .overlay(
                            RoundedRectangle(cornerRadius: 8)
                                .strokeBorder(Tokens.textGhost, lineWidth: 1)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help(T("sidebar.settings.help"))
                .accessibilityLabel(T("sidebar.settings.a11y"))
                .keyboardShortcut(",", modifiers: [.command])
            }

            // Live AEC meters — TTS render (top, accent) and mic capture
            // (bottom, green). Mirrors the TUI's sidebar bars; driven by
            // `host.ttsLevels` / `host.micLevels`.
            VStack(spacing: 4) {
                meterRow(label: "TTS",
                         levels: ttsLevels,
                         color: accent.main)
                meterRow(label: "MIC",
                         levels: micLevels,
                         color: Color(red: 0.55, green: 0.85, blue: 0.65))
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
    }

    /// Play/pause button anchored in the sidebar footer next to the
    /// voice name. Lives here (and not inside the active `LibRow`)
    /// because the active row can scroll off-screen and the user still
    /// needs a visible control. The button stays tappable in every
    /// non-idle state (paused, finished, unknown) so the user can
    /// resume from any kind of interruption — previously `.unknown`
    /// (which the FFI mapping produces for rodio's "stopped" sink
    /// state after a short paused chunk) disabled the button and
    /// made "play doesn't play" — that bug is gone.
    private var footerPlayButton: some View {
        let state = playbackState
        let noSession = (state == .idle)
        let dim = noSession
        return Button(action: onTogglePlay) {
            ZStack {
                Circle()
                    .fill(dim ? Color.white.opacity(0.05) : accent.main)
                    .shadow(color: dim ? .clear : accent.glow, radius: 5)
                Circle()
                    .strokeBorder(dim ? Tokens.textGhost : accent.main, lineWidth: 1)
                if state == .playing {
                    HStack(spacing: 2) {
                        Rectangle().fill(Tokens.bg).frame(width: 2, height: 9)
                        Rectangle().fill(Tokens.bg).frame(width: 2, height: 9)
                    }
                } else {
                    Path { p in
                        p.move(to: CGPoint(x: 9, y: 7))
                        p.addLine(to: CGPoint(x: 9, y: 19))
                        p.addLine(to: CGPoint(x: 19, y: 13))
                        p.closeSubpath()
                    }
                    .fill(dim ? Tokens.textDim : Tokens.bg)
                    .frame(width: 26, height: 26)
                }
            }
            .frame(width: 26, height: 26)
        }
        .buttonStyle(.plain)
        .disabled(noSession)
        .accessibilityLabel(state == .playing
                            ? T("sidebar.play.pause.a11y")
                            : T("sidebar.play.resume.a11y"))
        .help(state == .playing
              ? T("sidebar.play.pause.help")
              : T("sidebar.play.resume.help"))
    }

    @ViewBuilder
    private func meterRow(label: String, levels: [Float], color: Color) -> some View {
        HStack(spacing: 6) {
            Text(label)
                .font(.mono(9))
                .tracking(1)
                .foregroundStyle(Tokens.textDim)
                .frame(width: 24, alignment: .leading)
            Waveform(
                // More bars + brighter rest colour for legibility against
                // the dark sidebar. Idle state (`dim` at 0.6) renders as
                // a visible strip instead of disappearing; active peaks
                // hit ~22pt tall so a real spike reads at a glance.
                count: 48,
                accent: color,
                dim: color.opacity(0.6),
                seed: 0.7, minHeight: 4, maxBump: 18,
                liveLevels: levels
            )
            .frame(height: 22)
            .frame(maxWidth: .infinity)
        }
    }
}

struct SideSection<Content: View>: View {
    var title: String
    var accent: Accent?
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 6) {
                if let a = accent {
                    // Tiny accent dot leading the section title so the theme
                    // colour shows up even on sections without an active row.
                    Circle()
                        .fill(a.main.opacity(0.55))
                        .frame(width: 4, height: 4)
                }
                Text(title)
                    .font(.mono(10))
                    .tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
            }
            .padding(.horizontal, 18).padding(.top, 8).padding(.bottom, 6)
            content
        }
        .padding(.vertical, 6)
    }
}

struct SideRow: View {
    var label: String
    var count: Int
    var active: Bool
    var accent: Accent
    var body: some View {
        HStack {
            Text(label)
                .font(.serif(14, italic: active))
                .foregroundStyle(active ? Tokens.text : Tokens.textDim)
            Spacer()
            Text("\(count)")
                .font(.mono(10))
                .foregroundStyle(active ? accent.main : Tokens.textFaint)
        }
        .padding(.horizontal, 10).padding(.vertical, 6)
        // Active row gets a soft wash of the accent hue so theme changes
        // are immediately visible on the sidebar, not just on tiny accents.
        .background(active ? accent.soft : Color.clear)
        .overlay(alignment: .leading) {
            Rectangle()
                .fill(active ? accent.main : .clear)
                .frame(width: 2)
        }
        .padding(.horizontal, 8)
    }
}

struct LibRow: View {
    var entry: LibraryEntry
    var accent: Accent

    /// Compact, real metadata line: "N capitoli · M chunk · K note". The
    /// three parts each drop out when they'd be 0 so a fresh-ingested doc
    /// doesn't say "0 note" next to 214 chunks. No percentage anymore —
    /// `progressPct` wasn't wired to real data and read as mock.
    private var metadataLine: String {
        var parts: [String] = []
        if entry.chapterCount > 0 {
            parts.append(LibRow.pluralPart(
                count: entry.chapterCount,
                oneKey: "sidebar.library.chapters_one",
                otherKey: "sidebar.library.chapters_other"))
        }
        if entry.chunkCount > 0 {
            parts.append(LibRow.pluralPart(
                count: entry.chunkCount,
                oneKey: "sidebar.library.chunks_one",
                otherKey: "sidebar.library.chunks_other"))
        }
        if entry.notes > 0 {
            parts.append(LibRow.pluralPart(
                count: entry.notes,
                oneKey: "sidebar.library.notes_one",
                otherKey: "sidebar.library.notes_other"))
        }
        return parts.joined(separator: " · ")
    }

    /// Pick `_one` for 1, `_other` (with `%lld` substitution) otherwise.
    /// Keeps grammar correct in IT/EN without a `.stringsdict` file.
    private static func pluralPart(count: Int, oneKey: String, otherKey: String) -> String {
        count == 1 ? T(oneKey) : String(format: T(otherKey), count)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(entry.title)
                .font(.serif(15, italic: entry.active))
                .foregroundStyle(entry.active ? Tokens.text : Tokens.textDim)
                .lineLimit(1)
            HStack(spacing: 6) {
                if !metadataLine.isEmpty {
                    Text(metadataLine)
                        .font(.mono(9))
                        .tracking(0.3)
                        .foregroundStyle(Tokens.textFaint)
                }
                if entry.needsReload {
                    // Pure indicator — not a tap target. The reload
                    // action is in the row's context menu so we don't
                    // re-ingest on accidental clicks.
                    Image(systemName: "arrow.triangle.2.circlepath")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(Tokens.textFaint)
                        .help(T("sidebar.library.modified-on-disk"))
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10).padding(.vertical, 8)
        // Accent-washed bg on the active document, so the current reading
        // context is always visible and the theme colour reads across the
        // sidebar — not just on hairline accents.
        .background(entry.active ? accent.soft : Color.clear)
        .overlay(alignment: .leading) {
            if entry.active {
                Rectangle()
                    .fill(accent.main)
                    .frame(width: 2)
                    .padding(.vertical, 10)
                    .shadow(color: accent.glow, radius: 5)
            }
        }
        .padding(.horizontal, 8)
    }
}
