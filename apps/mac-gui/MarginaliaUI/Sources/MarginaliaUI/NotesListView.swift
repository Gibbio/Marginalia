import SwiftUI

/// Sheet listing all voice-dictated notes for the active document. Shows
/// them in reverse chronological order (most recent first); click → seek
/// to the note's chunk. Bookmarks are excluded — they live in their own
/// `BookmarkListView` because the UX target is different (lookup by
/// position vs. read-through).
public struct NotesListView: View {
    public let notes: [MarginNote]
    public var accent: Accent
    public var onSelect: (Int, Int) -> Void   // (section, chunk)
    public var onDelete: (String) -> Void
    public var onDismiss: () -> Void

    public init(
        notes: [MarginNote],
        accent: Accent,
        onSelect: @escaping (Int, Int) -> Void,
        onDelete: @escaping (String) -> Void,
        onDismiss: @escaping () -> Void
    ) {
        self.notes = notes
        self.accent = accent
        self.onSelect = onSelect
        self.onDelete = onDelete
        self.onDismiss = onDismiss
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Tokens.line)
            if notes.isEmpty {
                emptyState
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(notes) { n in
                            row(n)
                            Divider().overlay(Tokens.lineSoft)
                        }
                    }
                }
            }
        }
        .frame(width: 560, height: 520)
        .background(Tokens.bg)
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("NOTE")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("\(notes.count) not\(notes.count == 1 ? "a" : "e") in questo documento")
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
            Text("Nessuna nota")
                .font(.serif(18, italic: true))
                .foregroundStyle(Tokens.text)
            Text("Di' \"nota\" o premi ⌘N per dettarne una alla posizione corrente.")
                .font(.serif(13, italic: true))
                .foregroundStyle(Tokens.textDim)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
            Spacer()
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    @ViewBuilder
    private func row(_ n: MarginNote) -> some View {
        // Reuse the bookmark anchor parser — same format, different list.
        let pos = BookmarkListView.parseAnchor(n.chunkId)
        Button(action: {
            if let p = pos {
                onSelect(p.section, p.chunk)
                onDismiss()
            }
        }) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: "quote.opening")
                    .font(.system(size: 10))
                    .foregroundStyle(accent.main.opacity(0.7))
                    .frame(width: 18)
                    .padding(.top, 3)
                VStack(alignment: .leading, spacing: 6) {
                    Text(n.body)
                        .font(.serif(14))
                        .foregroundStyle(Tokens.text)
                        .lineSpacing(3)
                        .lineLimit(4)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .multilineTextAlignment(.leading)
                    HStack(spacing: 10) {
                        if let p = pos {
                            Text("cap \(p.section + 1) · chunk \(p.chunk + 1)")
                                .font(.mono(10))
                                .foregroundStyle(Tokens.textFaint)
                        }
                        Text(n.when)
                            .font(.mono(10))
                            .foregroundStyle(Tokens.textFaint)
                        if !n.status.isEmpty {
                            Text(n.status.uppercased())
                                .font(.mono(9)).tracking(0.8)
                                .foregroundStyle(accent.main)
                        }
                    }
                }
                Spacer()
                Button(action: { onDelete(n.id) }) {
                    Image(systemName: "trash")
                        .font(.system(size: 10))
                        .foregroundStyle(Color.red.opacity(0.75))
                        .padding(6)
                }
                .buttonStyle(.plain)
                .help("Elimina nota")
            }
            .padding(.horizontal, 20).padding(.vertical, 14)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
