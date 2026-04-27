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

    /// Lowercased union of every default trigger for a given action
    /// across every supported language. The customs view subtracts
    /// this from the stored `triggers` list — that way a word which
    /// is a default in some other language doesn't pop up as "user
    /// custom" after the user switches interface language. The word
    /// is still in the TOML and the STT resolver keeps recognizing
    /// it (so existing muscle memory keeps working); the UI just
    /// stops misattributing it.
    public static func allLanguageTriggers(action: String) -> Set<String> {
        var out = Set<String>()
        for (_, perAction) in table {
            if let list = perAction[action] {
                for w in list { out.insert(w.lowercased()) }
            }
        }
        return out
    }

    /// Lowercased union of every default trigger across every action
    /// and every language — used by the conflict check so the user
    /// can't add a custom that's already someone else's default in
    /// any language.
    public static func allLanguageTriggersAcrossActions() -> [String: String] {
        var out: [String: String] = [:]  // word -> action
        for (_, perAction) in table {
            for (action, list) in perAction {
                for w in list { out[w.lowercased()] = action }
            }
        }
        return out
    }

    /// Trigger sets per (language, action). Order matters for display:
    /// the editor renders defaults in this exact order. Adding a new
    /// language is one entry; adding a new action is one key inside
    /// each language map. Each action's row label/order is owned by
    /// the host's `voiceCommands`, not this table.
    private static let table: [String: [String: [String]]] = [
        "it": [
            // Italian defaults are pure-Italian — no English fallbacks.
            // "stop" is kept because it's a loanword routinely used in
            // spoken Italian; "pause", "resume", "next", "back", "repeat"
            // are not.
            "pause":        ["pausa", "ferma"],
            "resume":       ["riprendi", "continua"],
            "next":         ["avanti", "prossimo"],
            "back":         ["indietro"],
            "repeat":       ["ripeti"],
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
