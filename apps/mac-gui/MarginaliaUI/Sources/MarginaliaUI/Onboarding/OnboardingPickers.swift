import SwiftUI

/// Post-download onboarding steps: pick the reading voice, the UI language,
/// and the theme. Each view is self-contained and writes its selection via
/// the supplied closures so the shell controller can persist (config TOML
/// for voice, UserDefaults for locale + theme).

// MARK: — VoiceSelectView

/// Radio-single-select over installed voices. Only installed voices are
/// shown here — this view assumes the user has already gone through the
/// download manager step. If the list is empty (user skipped), "continua"
/// stays enabled and the selection closure is never called.
public struct VoiceSelectView: View {
    public var accent: Accent
    public var voices: [InstallableAsset]
    /// Current voice id (from the FFI spec). Used to highlight a pre-
    /// selected row when re-entering this step.
    public var selectedId: String?
    public var onSelect: (String) -> Void
    public var onProceed: () -> Void
    public var onBack: () -> Void

    public init(
        accent: Accent,
        voices: [InstallableAsset],
        selectedId: String?,
        onSelect: @escaping (String) -> Void,
        onProceed: @escaping () -> Void,
        onBack: @escaping () -> Void
    ) {
        self.accent = accent
        self.voices = voices
        self.selectedId = selectedId
        self.onSelect = onSelect
        self.onProceed = onProceed
        self.onBack = onBack
    }

    @State private var localPick: String? = nil

    public var body: some View {
        let currentPick = localPick ?? selectedId
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("VOCE")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("Scegli la voce per la lettura")
                    .font(.serif(32, weight: .medium))
                    .kerning(-0.3)
                    .foregroundStyle(Tokens.text)
                Text("La voce che ti leggerà i documenti ad alta voce. Puoi cambiarla in qualsiasi momento dalle Impostazioni.")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(4)
                    .frame(maxWidth: 620, alignment: .leading)
            }

            if voices.isEmpty {
                emptyState
            } else {
                ScrollView {
                    VStack(spacing: 10) {
                        ForEach(voiceGroups, id: \.language) { group in
                            voiceGroupView(group, currentPick: currentPick)
                        }
                    }
                    .padding(.trailing, 4)
                }
            }

            HStack(spacing: 12) {
                Button(action: onProceed) {
                    Text("continua")
                        .font(.sans(14, weight: .medium))
                        .foregroundStyle(currentPick != nil ? Tokens.bg : Tokens.textDim)
                        .padding(.horizontal, 22).padding(.vertical, 9)
                        .background(
                            Capsule().fill(currentPick != nil ? accent.main : Color.white.opacity(0.04))
                        )
                        .overlay(
                            Capsule().strokeBorder(currentPick != nil ? accent.main : Tokens.line)
                        )
                }
                .buttonStyle(.plain)
                .disabled(currentPick == nil && !voices.isEmpty)

                Button(action: onBack) {
                    Text("indietro")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 88).padding(.vertical, 48)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tokens.bg)
    }

    private var emptyState: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Nessuna voce installata")
                .font(.serif(16, italic: true))
                .foregroundStyle(Tokens.textDim)
            Text("Torna al passo precedente per scaricarne una, oppure prosegui e imposta la voce più tardi.")
                .font(.mono(11))
                .foregroundStyle(Tokens.textFaint)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.white.opacity(0.02))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }

    @ViewBuilder
    private func voiceGroupView(
        _ group: (language: String, display: String, assets: [InstallableAsset]),
        currentPick: String?
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(group.display.uppercased())
                .font(.mono(10)).tracking(1.2)
                .foregroundStyle(Tokens.textFaint)
                .padding(.leading, 4)
            VStack(spacing: 0) {
                ForEach(Array(group.assets.enumerated()), id: \.element.id) { idx, voice in
                    voiceRow(voice, selected: currentPick == voiceIdKey(voice))
                    if idx < group.assets.count - 1 {
                        Divider().frame(height: 1).overlay(Tokens.lineSoft)
                    }
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
    }

    @ViewBuilder
    private func voiceRow(_ voice: InstallableAsset, selected: Bool) -> some View {
        Button(action: {
            let key = voiceIdKey(voice)
            localPick = key
            onSelect(key)
        }) {
            HStack(spacing: 14) {
                ZStack {
                    Circle()
                        .strokeBorder(selected ? accent.main : Tokens.line, lineWidth: 1.5)
                        .frame(width: 18, height: 18)
                    if selected {
                        Circle()
                            .fill(accent.main)
                            .frame(width: 10, height: 10)
                    }
                }
                Text(voice.label)
                    .font(.serif(15, italic: selected))
                    .foregroundStyle(Tokens.text)
                Spacer()
            }
            .padding(.horizontal, 16).padding(.vertical, 12)
            .contentShape(Rectangle())
            .background(selected ? accent.main.opacity(0.05) : Color.clear)
        }
        .buttonStyle(.plain)
    }

    /// The Rust runtime stores voice as the bare id (e.g. `if_sara`), but
    /// the catalog surface ids are prefixed `voice:`. Keep the picker keyed
    /// on the bare id so writing to `cfg.mlx.voice` is trivial.
    private func voiceIdKey(_ voice: InstallableAsset) -> String {
        voice.id.hasPrefix("voice:")
            ? String(voice.id.dropFirst("voice:".count))
            : voice.id
    }

    private var voiceGroups: [(language: String, display: String, assets: [InstallableAsset])] {
        let byLang = Dictionary(grouping: voices, by: { $0.language ?? "und" })
        let sysLang = (Locale.current.language.languageCode?.identifier ?? "en").lowercased()
        return byLang
            .map { (lang, items) -> (String, String, [InstallableAsset]) in
                (lang, Self.languageDisplay(lang), items.sorted { $0.label < $1.label })
            }
            .sorted { a, b in
                let aIsSys = a.0.lowercased().hasPrefix(sysLang)
                let bIsSys = b.0.lowercased().hasPrefix(sysLang)
                if aIsSys != bIsSys { return aIsSys }
                return a.1 < b.1
            }
    }

    /// Full BCP-47 lookup to avoid collapsing `en-US` and `en-GB` into a
    /// single "Inglese" row.
    private static func languageDisplay(_ bcp47: String) -> String {
        let full = bcp47.lowercased()
        switch full {
        case "en-us": return "Inglese (US)"
        case "en-gb": return "Inglese (UK)"
        case "pt-br": return "Portoghese (BR)"
        case "pt-pt": return "Portoghese (PT)"
        case "zh-cn": return "Cinese (mandarino)"
        case "zh-tw": return "Cinese (tradizionale)"
        default: break
        }
        let code = String(full.prefix(2))
        switch code {
        case "it": return "Italiano"
        case "en": return "Inglese"
        case "fr": return "Francese"
        case "de": return "Tedesco"
        case "es": return "Spagnolo"
        case "pt": return "Portoghese"
        case "ja": return "Giapponese"
        case "zh": return "Cinese"
        case "hi": return "Hindi"
        default:   return bcp47
        }
    }
}

// MARK: — UILanguageSelectView

/// UI locale picker. Marginalia currently ships with an Italian-only
/// interface; English is listed but marked "presto disponibile" so the
/// step is honest about its state. The picker writes the selection to
/// UserDefaults, where the rest of the app can read it once localization
/// arrives.
public struct UILanguageSelectView: View {
    public var accent: Accent
    public var selectedLocale: String
    public var onSelect: (String) -> Void
    public var onProceed: () -> Void
    public var onBack: () -> Void

    public init(
        accent: Accent,
        selectedLocale: String,
        onSelect: @escaping (String) -> Void,
        onProceed: @escaping () -> Void,
        onBack: @escaping () -> Void
    ) {
        self.accent = accent
        self.selectedLocale = selectedLocale
        self.onSelect = onSelect
        self.onProceed = onProceed
        self.onBack = onBack
    }

    private struct Option: Identifiable {
        let id: String
        let label: String
        let enabled: Bool
    }

    private let options: [Option] = [
        Option(id: "it", label: "Italiano", enabled: true),
        Option(id: "en", label: "English (presto disponibile)", enabled: false),
    ]

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("LINGUA DELL'INTERFACCIA")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("In che lingua vuoi vedere Marginalia?")
                    .font(.serif(32, weight: .medium))
                    .kerning(-0.3)
                    .foregroundStyle(Tokens.text)
                Text("Questa è la lingua dei menu, delle impostazioni e dei messaggi dell'app. Non influenza la lingua dei documenti che leggi: quelli vengono riconosciuti automaticamente.")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(4)
                    .frame(maxWidth: 620, alignment: .leading)
            }

            VStack(spacing: 0) {
                ForEach(Array(options.enumerated()), id: \.element.id) { idx, opt in
                    optionRow(opt)
                    if idx < options.count - 1 {
                        Divider().frame(height: 1).overlay(Tokens.lineSoft)
                    }
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))

            // Push the nav buttons to the bottom of the page so they land
            // in the same spot as the other onboarding steps (which have
            // ScrollView content that naturally expands to the bottom).
            // Without this, the two radio rows are so short that the
            // buttons float just below them — visually disconnected from
            // where the user expects the "continua" affordance to be.
            Spacer(minLength: 0)

            HStack(spacing: 12) {
                Button(action: onProceed) {
                    Text("continua")
                        .font(.sans(14, weight: .medium))
                        .foregroundStyle(Tokens.bg)
                        .padding(.horizontal, 22).padding(.vertical, 9)
                        .background(Capsule().fill(accent.main))
                }
                .buttonStyle(.plain)

                Button(action: onBack) {
                    Text("indietro")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 88).padding(.vertical, 48)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tokens.bg)
    }

    @ViewBuilder
    private func optionRow(_ opt: Option) -> some View {
        let selected = (opt.id == selectedLocale)
        Button(action: { if opt.enabled { onSelect(opt.id) } }) {
            HStack(spacing: 14) {
                ZStack {
                    Circle()
                        .strokeBorder(
                            selected ? accent.main : (opt.enabled ? Tokens.line : Tokens.lineSoft),
                            lineWidth: 1.5
                        )
                        .frame(width: 18, height: 18)
                    if selected {
                        Circle().fill(accent.main).frame(width: 10, height: 10)
                    }
                }
                Text(opt.label)
                    .font(.serif(15, italic: selected))
                    .foregroundStyle(opt.enabled ? Tokens.text : Tokens.textFaint)
                Spacer()
            }
            .padding(.horizontal, 16).padding(.vertical, 14)
            .contentShape(Rectangle())
            .background(selected ? accent.main.opacity(0.05) : Color.clear)
        }
        .buttonStyle(.plain)
        .disabled(!opt.enabled)
    }
}

// MARK: — ThemeSelectView

/// Theme picker with live preview. Selecting a theme calls `onSelect`
/// immediately so the parent can rebind the accent and the whole tree
/// repaints, giving the user a before/after comparison on the same view.
public struct ThemeSelectView: View {
    public var accent: Accent
    public var selectedThemeId: String
    public var onSelect: (String) -> Void
    public var onProceed: () -> Void
    public var onBack: () -> Void

    public init(
        accent: Accent,
        selectedThemeId: String,
        onSelect: @escaping (String) -> Void,
        onProceed: @escaping () -> Void,
        onBack: @escaping () -> Void
    ) {
        self.accent = accent
        self.selectedThemeId = selectedThemeId
        self.onSelect = onSelect
        self.onProceed = onProceed
        self.onBack = onBack
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("TEMA")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("Scegli il colore d'accento")
                    .font(.serif(32, weight: .medium))
                    .kerning(-0.3)
                    .foregroundStyle(Tokens.text)
                Text("Il colore che evidenzia titoli, bottoni e barre audio. Lo sfondo resta invariato — è parte dell'identità di Marginalia.")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(4)
                    .frame(maxWidth: 620, alignment: .leading)
            }

            HStack(spacing: 12) {
                ForEach(AppTheme.all, id: \.id) { theme in
                    themeCard(theme, selected: theme.id == selectedThemeId)
                }
                Spacer()
            }

            HStack(spacing: 12) {
                Button(action: onProceed) {
                    Text("fine")
                        .font(.sans(14, weight: .medium))
                        .foregroundStyle(Tokens.bg)
                        .padding(.horizontal, 22).padding(.vertical, 9)
                        .background(Capsule().fill(accent.main))
                }
                .buttonStyle(.plain)

                Button(action: onBack) {
                    Text("indietro")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 88).padding(.vertical, 48)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tokens.bg)
    }

    @ViewBuilder
    private func themeCard(_ theme: Theme, selected: Bool) -> some View {
        let cardAccent = Accent(hue: theme.accentHue)
        Button(action: { onSelect(theme.id) }) {
            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text(theme.display)
                        .font(.serif(16, weight: .medium, italic: selected))
                        .foregroundStyle(Tokens.text)
                    Spacer()
                    if selected {
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 14))
                            .foregroundStyle(cardAccent.main)
                    }
                }
                // Swatches — the accent palette in thumbnail form.
                HStack(spacing: 6) {
                    RoundedRectangle(cornerRadius: 4).fill(cardAccent.main).frame(width: 28, height: 14)
                    RoundedRectangle(cornerRadius: 4).fill(cardAccent.main.opacity(0.5)).frame(width: 20, height: 14)
                    RoundedRectangle(cornerRadius: 4).fill(cardAccent.main.opacity(0.2)).frame(width: 14, height: 14)
                }
                Text(theme.blurb)
                    .font(.serif(12, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(2)
                    .lineLimit(3, reservesSpace: true)
                    .frame(maxWidth: 200, alignment: .leading)
            }
            .padding(14)
            .frame(width: 220, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(selected ? cardAccent.main.opacity(0.08) : Color.white.opacity(0.02))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(selected ? cardAccent.main : Tokens.line, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
    }
}
