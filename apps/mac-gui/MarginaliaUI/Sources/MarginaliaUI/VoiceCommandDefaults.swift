import Foundation

/// Per-interface-language default trigger words for each voice command
/// action. The Settings editor renders these as locked, dimmed chips
/// distinct from user-added (custom) triggers.
///
/// Why per-language: the user's earlier feedback — they want to keep
/// the interface in one language while controlling reading via voice
/// in another, OR have the trigger set adapt when they switch UI
/// language. The defaults table is keyed by the InterfaceLanguage code
/// (`"it"` / `"en"`); custom triggers, on the other hand, persist in
/// the TOML across switches.
///
/// Why no Rust mirror (yet): the resolver consumes a flat trigger list
/// from `voice_commands.<action>`. The Mac UI is responsible for
/// keeping that list in sync (defaults ∪ customs) when the user edits;
/// the resolver doesn't need its own copy of the default table.
public enum VoiceCommandDefaults {

    /// Lookup `(language, action)` → default triggers. Returns an
    /// empty list when either is unknown — the editor then treats
    /// every stored trigger as custom, which is the conservative
    /// fallback (nothing shown as "blessed default").
    public static func triggers(for language: String, action: String) -> [String] {
        table[language]?[action] ?? []
    }

    /// All defaults for a language (across every action). Used by the
    /// editor's conflict check — a candidate "pausa" must clash with
    /// pause's defaults regardless of whether the row showing the
    /// conflict is the active one.
    public static func allTriggers(for language: String) -> [String] {
        guard let perAction = table[language] else { return [] }
        return perAction.values.flatMap { $0 }
    }

    /// Trigger sets per (language, action). Order matters for display:
    /// the editor renders defaults in this exact order. Adding a new
    /// language is one entry; adding a new action is one key inside
    /// each language map. Each action's row label/order is owned by
    /// the host's `voiceCommands`, not this table.
    private static let table: [String: [String: [String]]] = [
        "it": [
            "pause":        ["pausa", "ferma", "pause"],
            "resume":       ["riprendi", "continua", "resume"],
            "next":         ["avanti", "prossimo", "next"],
            "back":         ["indietro", "back"],
            "repeat":       ["ripeti", "repeat"],
            "stop":         ["stop", "basta"],
            "next_chapter": ["prossimo capitolo", "capitolo avanti"],
            "prev_chapter": ["capitolo precedente", "capitolo indietro"],
            "bookmark":     ["segna", "segnalibro"],
            "note":         ["nota", "appunto"],
            "where":        ["dove sono", "posizione"],
        ],
        "en": [
            "pause":        ["pause"],
            "resume":       ["resume", "continue"],
            "next":         ["next", "skip"],
            "back":         ["back", "previous"],
            "repeat":       ["repeat", "again"],
            "stop":         ["stop"],
            "next_chapter": ["next chapter"],
            "prev_chapter": ["previous chapter"],
            "bookmark":     ["bookmark", "mark"],
            "note":         ["note"],
            "where":        ["where", "position"],
        ],
    ]
}
