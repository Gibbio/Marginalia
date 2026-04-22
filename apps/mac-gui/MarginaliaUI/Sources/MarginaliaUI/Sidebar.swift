import SwiftUI

/// Left-hand library sidebar (264 px wide). Uses the injected `library`
/// slice (either the mock's static list or the real FFI host's @Published
/// `library`) so the component stays pure — no host binding here.
public struct Sidebar: View {
    public var accent: Accent
    public var library: [LibraryEntry]
    public var micLevels: [Float]
    public var ttsLevels: [Float]
    public var onAddDocument: () -> Void
    public var onOpenDocument: (String) -> Void

    public init(accent: Accent,
                library: [LibraryEntry],
                micLevels: [Float] = [],
                ttsLevels: [Float] = [],
                onAddDocument: @escaping () -> Void = {},
                onOpenDocument: @escaping (String) -> Void = { _ in }) {
        self.accent = accent
        self.library = library
        self.micLevels = micLevels
        self.ttsLevels = ttsLevels
        self.onAddDocument = onAddDocument
        self.onOpenDocument = onOpenDocument
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().frame(height: 1).overlay(Tokens.line)
            searchBarRow
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    SideSection(title: "raccolte", accent: accent) {
                        SideRow(label: "Tutto", count: 47, active: false, accent: accent)
                        SideRow(label: "In ascolto", count: 3, active: true, accent: accent)
                        SideRow(label: "Bozze", count: 4, active: false, accent: accent)
                        SideRow(label: "Archivio", count: 28, active: false, accent: accent)
                    }
                    SideSection(title: "libreria", accent: accent) {
                        ForEach(library) { entry in
                            Button(action: { onOpenDocument(entry.id) }) {
                                LibRow(entry: entry, accent: accent)
                            }
                            .buttonStyle(.plain)
                        }
                        if library.isEmpty {
                            Text("Nessun documento. Usa + per importare.")
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

    /// Header: just the wordmark. Gear moved to the margin panel footer,
    /// "+" moved next to the search bar — both consolidated with their
    /// semantically-related controls.
    private var header: some View {
        HStack {
            Text("Marginalia")
                .font(.serif(18, italic: true))
                .foregroundStyle(Tokens.text)
            Spacer()
        }
        // `.hiddenTitleBar` leaves the native traffic-light controls
        // floating at x≈20…80. The 82-pt leading padding clears them.
        .padding(.leading, 82)
        .padding(.trailing, 18)
        .frame(height: 52)
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
            }
            .buttonStyle(.plain)
            .help("Importa documento…")
            .accessibilityLabel("Importa documento")
        }
        .padding(.horizontal, 14).padding(.top, 12).padding(.bottom, 10)
    }

    /// Search input, used inside `searchBarRow` which also hosts the "+".
    private var searchBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 10))
                .foregroundStyle(Tokens.textFaint)
            Text("cerca o di'…")
                .font(.sans(12))
                .foregroundStyle(Tokens.textFaint)
            Spacer(minLength: 8)
            Text("⌘K")
                .font(.mono(10))
                .foregroundStyle(Tokens.textFaint)
                .padding(.horizontal, 5).padding(.vertical, 2)
                .background(RoundedRectangle(cornerRadius: 4).fill(Color.white.opacity(0.04)))
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
                ZStack {
                    Circle().strokeBorder(Tokens.textGhost, lineWidth: 1)
                    Circle().fill(accent.main).frame(width: 5, height: 5).shadow(color: accent.main, radius: 5)
                }
                .frame(width: 26, height: 26)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Voce: Elena")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.text)
                    Text("IT · 1.0×")
                        .font(.mono(9))
                        .foregroundStyle(Tokens.textFaint)
                }
                Spacer()
            }

            // Live AEC meters — TTS render (top, accent) and mic capture
            // (bottom, green). Mirrors the TUI's sidebar bars; driven by
            // `host.ttsLevels` / `host.micLevels`.
            VStack(spacing: 3) {
                meterRow(label: "TTS",
                         levels: ttsLevels,
                         color: accent.main.opacity(0.85))
                meterRow(label: "MIC",
                         levels: micLevels,
                         color: Color(red: 0.45, green: 0.75, blue: 0.55))
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
    }

    @ViewBuilder
    private func meterRow(label: String, levels: [Float], color: Color) -> some View {
        HStack(spacing: 6) {
            Text(label)
                .font(.mono(8))
                .tracking(1)
                .foregroundStyle(Tokens.textFaint)
                .frame(width: 22, alignment: .leading)
            Waveform(
                count: 32,
                accent: color,
                dim: color.opacity(0.25),
                seed: 0.7, minHeight: 2, maxBump: 10,
                liveLevels: levels
            )
            .frame(height: 12)
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
    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                Text(entry.title)
                    .font(.serif(15, italic: entry.active))
                    .foregroundStyle(entry.active ? Tokens.text : Tokens.textDim)
                    .lineLimit(1)
                Text("\(entry.subtitle) · \(entry.progressPct)%")
                    .font(.mono(9))
                    .tracking(0.3)
                    .foregroundStyle(Tokens.textFaint)
                    .padding(.bottom, 5)
                if entry.progressPct > 0 {
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Rectangle().fill(Tokens.line).frame(height: 1)
                            Rectangle()
                                .fill(entry.active ? accent.main : Color(hex: 0xEFE5CF, opacity: 0.35))
                                .frame(width: g.size.width * CGFloat(entry.progressPct) / 100.0, height: 1)
                        }
                    }
                    .frame(height: 1)
                }
            }
            if entry.notes > 0 {
                Text("\(entry.notes)")
                    .font(.mono(9))
                    .foregroundStyle(entry.active ? accent.main : Tokens.textFaint)
                    .padding(.top, 2)
            }
        }
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
