import SwiftUI

/// Sheet listing all bookmarks for the active document. Bookmarks are
/// notes with the `[BOOKMARK]` prefix — the `bookmark()` host method
/// writes them that way, and this view extracts the trailing label +
/// anchor-derived position for display + seek.
///
/// Triggered from `Lettura → Lista segnalibri` (⌘⌥B) in the menu bar.
public struct BookmarkListView: View {
    public let bookmarks: [MarginNote]
    public var accent: Accent
    public var onSelect: (Int, Int) -> Void   // (section, chunk)
    public var onDismiss: () -> Void

    public init(
        bookmarks: [MarginNote],
        accent: Accent,
        onSelect: @escaping (Int, Int) -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.bookmarks = bookmarks
        self.accent = accent
        self.onSelect = onSelect
        self.onDismiss = onDismiss
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Tokens.line)
            if bookmarks.isEmpty {
                emptyState
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(bookmarks) { b in
                            row(b)
                            Divider().overlay(Tokens.lineSoft)
                        }
                    }
                }
            }
        }
        .frame(width: 480, height: 420)
        .background(Tokens.bg)
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("SEGNALIBRI")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("\(bookmarks.count) posizion\(bookmarks.count == 1 ? "e" : "i") salvat\(bookmarks.count == 1 ? "a" : "e")")
                    .font(.serif(14, italic: true))
                    .foregroundStyle(Tokens.textDim)
            }
            Spacer()
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Tokens.textFaint)
                    .padding(6)
            }
            .buttonStyle(.plain)
            .keyboardShortcut(.cancelAction)
        }
        .padding(.horizontal, 20).padding(.vertical, 16)
    }

    private var emptyState: some View {
        VStack(spacing: 10) {
            Spacer()
            Text("Nessun segnalibro")
                .font(.serif(18, italic: true))
                .foregroundStyle(Tokens.text)
            Text("Di' \"segnalibro\" o premi ⌘B per salvarne uno alla posizione corrente.")
                .font(.serif(13, italic: true))
                .foregroundStyle(Tokens.textDim)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 340)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private func row(_ b: MarginNote) -> some View {
        let pos = Self.parseAnchor(b.chunkId)
        Button(action: {
            if let p = pos {
                onSelect(p.section, p.chunk)
                onDismiss()
            }
        }) {
            HStack(spacing: 14) {
                Image(systemName: "bookmark.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(accent.main)
                    .frame(width: 22)
                VStack(alignment: .leading, spacing: 2) {
                    Text(Self.displayLabel(b.body))
                        .font(.serif(14))
                        .foregroundStyle(Tokens.text)
                        .lineLimit(1)
                    HStack(spacing: 6) {
                        if let p = pos {
                            Text("cap \(p.section + 1) · chunk \(p.chunk + 1)")
                                .font(.mono(10))
                                .foregroundStyle(Tokens.textFaint)
                        }
                        Text(b.when)
                            .font(.mono(10))
                            .foregroundStyle(Tokens.textFaint)
                    }
                }
                Spacer()
                if pos != nil {
                    Image(systemName: "arrow.right")
                        .font(.system(size: 10))
                        .foregroundStyle(Tokens.textFaint)
                }
            }
            .padding(.horizontal, 20).padding(.vertical, 12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(pos == nil)
    }

    /// Strip the `[BOOKMARK]` prefix added by `host.bookmark()` so the
    /// row shows the human-readable label instead of the machine tag.
    static func displayLabel(_ body: String) -> String {
        let trimmed = body.trimmingCharacters(in: .whitespaces)
        if trimmed.hasPrefix("[BOOKMARK]") {
            return String(trimmed.dropFirst("[BOOKMARK]".count))
                .trimmingCharacters(in: .whitespaces)
        }
        return trimmed
    }

    /// Parse `section:N/chunk:M` → (N, M). Returns nil when the anchor
    /// doesn't match the expected format (e.g. an older bookmark from a
    /// pre-anchor build).
    static func parseAnchor(_ anchor: String) -> (section: Int, chunk: Int)? {
        let parts = anchor.split(separator: "/")
        guard parts.count == 2,
              let sec = Self.extractInt(from: String(parts[0]), prefix: "section:"),
              let ck = Self.extractInt(from: String(parts[1]), prefix: "chunk:")
        else { return nil }
        return (sec, ck)
    }

    private static func extractInt(from s: String, prefix: String) -> Int? {
        guard s.hasPrefix(prefix) else { return nil }
        return Int(s.dropFirst(prefix.count))
    }
}

public extension MarginNote {
    /// True when this note is a bookmark (prefix-tagged by the host's
    /// `bookmark()` method).
    var isBookmark: Bool {
        body.trimmingCharacters(in: .whitespaces).hasPrefix("[BOOKMARK]")
    }
}
