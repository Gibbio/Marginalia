import SwiftUI

/// Download manager for the onboarding flow. Shows every installable
/// asset grouped into two sections:
///
///   • Engines (mlx-core, whisper-small, kokoro-onnx) — install + remove
///   • Voices, grouped by language, system-language expanded first — install
///     only (voices are managed as a single bundle with the engine; removing
///     a single voice is intentionally not exposed here)
///
/// The view is self-contained: it reads from `host.installations` and
/// `host.inflightDownloads`, and calls back through the supplied closures
/// so the same component can drive both onboarding (skip/continue buttons)
/// and Settings (no skip/continue, lives inside a section).
public struct DownloadManagerView: View {
    public var accent: Accent
    public var installations: [InstallableAsset]
    public var inflightStates: [String: InstallUiState]
    public var onInstall: (String) -> Void
    public var onUninstall: (String) -> Void
    public var onProceed: () -> Void
    public var onSkip: () -> Void

    public init(
        accent: Accent,
        installations: [InstallableAsset],
        inflightStates: [String: InstallUiState],
        onInstall: @escaping (String) -> Void,
        onUninstall: @escaping (String) -> Void,
        onProceed: @escaping () -> Void,
        onSkip: @escaping () -> Void
    ) {
        self.accent = accent
        self.installations = installations
        self.inflightStates = inflightStates
        self.onInstall = onInstall
        self.onUninstall = onUninstall
        self.onProceed = onProceed
        self.onSkip = onSkip
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            header
            if installations.isEmpty {
                loadingState
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 20) {
                        if !engineAssets.isEmpty {
                            engineSection
                        }
                        if !voiceGroups.isEmpty {
                            voicesSection
                        }
                        networkBanner
                    }
                    .padding(.trailing, 4)  // room for scrollbar
                }
            }
            footer
        }
        .padding(.horizontal, 88).padding(.vertical, 48)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tokens.bg)
    }

    /// Shown on the first frame after the view appears and `refreshInstallations`
    /// is still in flight. `host.installations` starts as `[]` — the moment the
    /// Rust side returns the catalog, this branch swaps out for the real rows.
    private var loadingState: some View {
        VStack(spacing: 14) {
            ProgressView()
                .progressViewStyle(.circular)
                .controlSize(.large)
                .tint(accent.main)
            Text("carico il catalogo…")
                .font(.mono(11)).tracking(1)
                .foregroundStyle(Tokens.textFaint)
            Text("sto leggendo il manifest e controllando quali file sono già in cache sul disco.")
                .font(.serif(12, italic: true))
                .foregroundStyle(Tokens.textFaint)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 360)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .padding(40)
    }

    // MARK: — Sections

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("DOWNLOAD MANAGER")
                .font(.mono(10)).tracking(2)
                .foregroundStyle(Tokens.textFaint)
            Text("Scarica i modelli e scegli le voci")
                .font(.serif(32, weight: .medium))
                .kerning(-0.3)
                .foregroundStyle(Tokens.text)
            Text("Marginalia usa un motore TTS (Kokoro MLX, ~310 MB) e almeno una voce per leggere ad alta voce. Puoi proseguire anche senza — sentirai il testo solo quando avrai installato il motore.")
                .font(.serif(15, italic: true))
                .foregroundStyle(Tokens.textDim)
                .lineSpacing(4)
                .frame(maxWidth: 620, alignment: .leading)
        }
    }

    private var engineSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("MOTORI")
            VStack(spacing: 0) {
                ForEach(Array(engineAssets.enumerated()), id: \.element.id) { idx, asset in
                    InstallRow(
                        item: asset,
                        accent: accent,
                        state: inflightStates[asset.id],
                        onInstall: onInstall,
                        onUninstall: onUninstall
                    )
                    if idx < engineAssets.count - 1 {
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

    private var voicesSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            sectionLabel("VOCI")
            VStack(spacing: 10) {
                ForEach(voiceGroups, id: \.language) { group in
                    voiceGroupView(group)
                }
            }
        }
    }

    @ViewBuilder
    private func voiceGroupView(
        _ group: (language: String, display: String, assets: [InstallableAsset])
    ) -> some View {
        let installedCount = group.assets.filter { $0.installed }.count
        let missing = group.assets.filter { !$0.installed }
        let expanded = Binding<Bool>(
            get: { expandedLangs.contains(group.language) },
            set: { on in
                if on { expandedLangs.insert(group.language) }
                else  { expandedLangs.remove(group.language) }
            }
        )
        DisclosureGroup(isExpanded: expanded) {
            VStack(spacing: 0) {
                ForEach(Array(group.assets.enumerated()), id: \.element.id) { idx, asset in
                    // Voices don't surface the "rimuovi" button — pass an
                    // empty callback so InstallRow's `removable` gate
                    // (which we set to false via InstallableAsset) hides it.
                    InstallRow(
                        item: asset,
                        accent: accent,
                        state: inflightStates[asset.id],
                        onInstall: onInstall,
                        onUninstall: { _ in }
                    )
                    if idx < group.assets.count - 1 {
                        Divider().frame(height: 1).overlay(Tokens.lineSoft)
                    }
                }
            }
            .padding(.top, 4)
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        } label: {
            HStack {
                Text(group.display)
                    .font(.serif(14, italic: installedCount > 0))
                    .foregroundStyle(Tokens.text)
                Text("(\(installedCount)/\(group.assets.count))")
                    .font(.mono(10))
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                if !missing.isEmpty {
                    Button("installa tutte (\(missing.count))") {
                        for a in missing { onInstall(a.id) }
                    }
                    .buttonStyle(.plain)
                    .font(.mono(11))
                    .foregroundStyle(accent.main)
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 5)
                        .fill(accent.main.opacity(0.08)))
                    .overlay(RoundedRectangle(cornerRadius: 5)
                        .strokeBorder(accent.main.opacity(0.3), lineWidth: 1))
                }
            }
            .padding(.vertical, 6)
        }
        .tint(Tokens.textFaint)
    }

    private func sectionLabel(_ text: String) -> some View {
        Text(text)
            .font(.mono(10)).tracking(1.5)
            .foregroundStyle(Tokens.textFaint)
    }

    private var footer: some View {
        HStack(spacing: 12) {
            Button(action: onProceed) {
                Text(canProceed ? "continua" : "continua comunque")
                    .font(.sans(14, weight: .medium))
                    .foregroundStyle(canProceed ? Tokens.bg : Tokens.textDim)
                    .padding(.horizontal, 22).padding(.vertical, 9)
                    .background(
                        Capsule().fill(canProceed ? accent.main : Color.white.opacity(0.04))
                    )
                    .overlay(
                        Capsule().strokeBorder(canProceed ? accent.main : Tokens.line)
                    )
            }
            .buttonStyle(.plain)

            Button(action: onSkip) {
                Text("salta per ora")
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .padding(.horizontal, 10).padding(.vertical, 6)
            }
            .buttonStyle(.plain)
        }
    }

    // MARK: — Derived

    @State private var expandedLangs: Set<String> = []

    /// Engines — anything with category != "voice". Order follows the
    /// order the catalog produces (mlx-core first, then whisper, then onnx).
    private var engineAssets: [InstallableAsset] {
        installations.filter { $0.category != "voice" }
    }

    /// Voices grouped by BCP-47 language. System language first, then
    /// alphabetical by display name. Each group's assets stay in catalog
    /// order for stability.
    private var voiceGroups: [(language: String, display: String, assets: [InstallableAsset])] {
        let voices = installations.filter { $0.category == "voice" }
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

    /// Proceed enabled once the primary TTS engine is installed and at
    /// least one voice is available. Otherwise the button label flips to
    /// "continua comunque" to warn the user.
    private var canProceed: Bool {
        let hasEngine = installations.contains {
            $0.category == "tts_core" && $0.id == "mlx-core" && $0.installed
        }
        let hasVoice = installations.contains {
            $0.category == "voice" && $0.installed
        }
        return hasEngine && hasVoice
    }

    /// Full BCP-47 lookup so `en-US` and `en-GB` don't collapse into the
    /// same "Voci inglesi" row. Falls back on the 2-letter prefix, then
    /// on the raw tag when we don't have a translation.
    private static func languageDisplay(_ bcp47: String) -> String {
        let full = bcp47.lowercased()
        switch full {
        case "en-us": return "Voci inglesi (US)"
        case "en-gb": return "Voci inglesi (UK)"
        case "pt-br": return "Voci portoghesi (BR)"
        case "pt-pt": return "Voci portoghesi (PT)"
        case "zh-cn": return "Voci cinesi (mandarino)"
        case "zh-tw": return "Voci cinesi (tradizionale)"
        default: break
        }
        let code = String(full.prefix(2))
        switch code {
        case "it": return "Voci italiane"
        case "en": return "Voci inglesi"
        case "fr": return "Voci francesi"
        case "de": return "Voci tedesche"
        case "es": return "Voci spagnole"
        case "pt": return "Voci portoghesi"
        case "ja": return "Voci giapponesi"
        case "zh": return "Voci cinesi"
        case "hi": return "Voci hindi"
        default:   return "Voci — \(bcp47)"
        }
    }

    private var networkBanner: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "network")
                .font(.system(size: 11))
                .foregroundStyle(Tokens.textFaint)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text("L'UNICO MOMENTO IN CUI SI USA LA RETE")
                    .font(.mono(10)).tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                Text("I file arrivano da huggingface.co. Dopo il download, Marginalia funziona completamente offline — niente altri collegamenti di rete.")
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(3)
            }
        }
        .padding(14)
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
}
