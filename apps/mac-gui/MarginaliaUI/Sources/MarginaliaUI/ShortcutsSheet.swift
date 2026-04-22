import SwiftUI

/// `Aiuto → Scorciatoie` cheatsheet. Two columns of (label, keycap) rows
/// grouped by category — keyboard first, voice commands second. Kept
/// static to avoid duplicating the shortcut definitions across the
/// menu bar and this view; when shortcuts drift, update both.
public struct ShortcutsSheet: View {
    public var accent: Accent
    public var onDismiss: () -> Void

    public init(accent: Accent, onDismiss: @escaping () -> Void) {
        self.accent = accent
        self.onDismiss = onDismiss
    }

    public var body: some View {
        VStack(spacing: 0) {
            header
            Divider().overlay(Tokens.line)
            ScrollView {
                VStack(alignment: .leading, spacing: 28) {
                    section("TASTIERA — LETTURA",
                            items: keyboardReading)
                    section("TASTIERA — NOTE E SEGNALIBRI",
                            items: keyboardNotes)
                    section("TASTIERA — GENERALE",
                            items: keyboardGeneral)
                    section("COMANDI VOCALI",
                            items: voice)
                }
                .padding(.horizontal, 24).padding(.vertical, 20)
            }
        }
        .frame(width: 640, height: 620)
        .background(Tokens.bg)
    }

    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text("SCORCIATOIE")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("Guida rapida")
                    .font(.serif(22, italic: true))
                    .foregroundStyle(Tokens.text)
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
        .padding(.horizontal, 24).padding(.vertical, 16)
    }

    @ViewBuilder
    private func section(_ title: String, items: [(String, String)]) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.mono(10)).tracking(1.5)
                .foregroundStyle(Tokens.textFaint)
            VStack(spacing: 0) {
                ForEach(Array(items.enumerated()), id: \.offset) { idx, pair in
                    HStack {
                        Text(pair.0)
                            .font(.serif(14))
                            .foregroundStyle(Tokens.text)
                        Spacer()
                        keycap(pair.1)
                    }
                    .padding(.horizontal, 14).padding(.vertical, 9)
                    if idx < items.count - 1 {
                        Divider().overlay(Tokens.lineSoft)
                    }
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
    }

    @ViewBuilder
    private func keycap(_ text: String) -> some View {
        Text(text)
            .font(.mono(11))
            .foregroundStyle(Tokens.textDim)
            .padding(.horizontal, 7).padding(.vertical, 3)
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .fill(Color.white.opacity(0.04))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
    }

    // MARK: — Shortcut definitions

    private var keyboardReading: [(String, String)] {
        [
            ("Pausa / Riprendi",       "␣"),
            ("Chunk successivo",       "⌘→"),
            ("Chunk precedente",       "⌘←"),
            ("Ripeti chunk",           "⌘R"),
            ("Capitolo successivo",    "⌘⇧→"),
            ("Capitolo precedente",    "⌘⇧←"),
            ("Volume più alto",        "⌘↑"),
            ("Volume più basso",       "⌘↓"),
            ("Dove sono",              "⌘?"),
        ]
    }

    private var keyboardNotes: [(String, String)] {
        [
            ("Nuova nota dettata",     "⌘N"),
            ("Elenco note",            "⌘⌥N"),
            ("Salva segnalibro",       "⌘B"),
            ("Lista segnalibri",       "⌘⌥B"),
            ("Esporta note…",          "⌘⇧E"),
        ]
    }

    private var keyboardGeneral: [(String, String)] {
        [
            ("Importa documento…",     "⌘O"),
            ("Importa da URL…",        "⌘U"),
            ("Impostazioni",           "⌘,"),
            ("Mostra / nascondi log",  "⌥L"),
            ("Chiudi sessione",        "⌘W"),
        ]
    }

    /// Voice commands are the default triggers shipped with Italian. The
    /// user can override them in Settings → Comandi vocali, but this
    /// cheatsheet always shows the defaults as a reference starting point.
    private var voice: [(String, String)] {
        [
            ("Pausa",                  "\"pausa\""),
            ("Riprendi",               "\"riprendi\""),
            ("Chunk successivo",       "\"prossimo\""),
            ("Chunk precedente",       "\"indietro\""),
            ("Ripeti",                 "\"ripeti\""),
            ("Capitolo successivo",    "\"prossimo capitolo\""),
            ("Capitolo precedente",    "\"capitolo precedente\""),
            ("Salva segnalibro",       "\"segna\" / \"segnalibro\""),
            ("Nuova nota dettata",     "\"nota\""),
            ("Dove sono",              "\"dove\""),
            ("Stop",                   "\"ferma\" / \"stop\""),
        ]
    }
}
