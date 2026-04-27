import SwiftUI

// MARK: — Language picker (3-column pill grid)

public struct LangPicker: View {
    public var languages: [LangInfo]
    public var selection: String
    public var accent: Accent
    public var onChange: (String) -> Void

    public init(languages: [LangInfo], selection: String, accent: Accent,
                onChange: @escaping (String) -> Void) {
        self.languages = languages; self.selection = selection
        self.accent = accent; self.onChange = onChange
    }

    public var body: some View {
        let cols = Array(repeating: GridItem(.flexible(), spacing: 8), count: 3)
        LazyVGrid(columns: cols, spacing: 8) {
            ForEach(languages) { lang in
                let active = selection == lang.bcp47
                Button(action: { onChange(lang.bcp47) }) {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(lang.display)
                            .font(.serif(17, italic: active))
                            .foregroundStyle(active ? Tokens.text : Tokens.textDim)
                        HStack {
                            Text(lang.bcp47)
                                .font(.mono(10))
                                .foregroundStyle(Tokens.textFaint)
                            Spacer()
                            Text("\(lang.voiceCount) voci")
                                .font(.mono(10))
                                .foregroundStyle(active ? accent.main : Tokens.textFaint)
                        }
                    }
                    .padding(.horizontal, 14).padding(.vertical, 12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(active ? Color.white.opacity(0.05) : Color.white.opacity(0.015))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .strokeBorder(active ? accent.main : Tokens.line, lineWidth: 1)
                    )
                    .shadow(color: active ? accent.glow : .clear, radius: 9)
                }
                .buttonStyle(.plain)
            }
        }
    }
}

// MARK: — Voice picker (list with gender + installed state)

public struct VoicePicker: View {
    public var voices: [VoiceInfo]
    @Binding public var selection: String
    public var accent: Accent

    public init(voices: [VoiceInfo], selection: Binding<String>, accent: Accent) {
        self.voices = voices; self._selection = selection; self.accent = accent
    }

    public var body: some View {
        if voices.isEmpty {
            Text("nessuna voce disponibile")
                .font(.serif(14, italic: true))
                .foregroundStyle(Tokens.textFaint)
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .strokeBorder(Tokens.line, lineWidth: 1)
                )
        } else {
            VStack(spacing: 0) {
                ForEach(Array(voices.enumerated()), id: \.element.id) { idx, voice in
                    voiceRow(voice)
                    if idx < voices.count - 1 {
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
    private func voiceRow(_ v: VoiceInfo) -> some View {
        let active = selection == v.id
        Button(action: { if v.installed { selection = v.id } }) {
            HStack(spacing: 14) {
                GenderIcon(gender: v.gender)
                VStack(alignment: .leading, spacing: 2) {
                    Text(v.display)
                        .font(.serif(17, italic: active))
                        .foregroundStyle(Tokens.text)
                    Text("\(v.id) · \(v.backend)")
                        .font(.mono(10))
                        .tracking(0.3)
                        .foregroundStyle(Tokens.textFaint)
                }
                Spacer()
                if !v.installed {
                    Text("non installata")
                        .font(.mono(10))
                        .foregroundStyle(Tokens.textFaint)
                        .padding(.horizontal, 8).padding(.vertical, 3)
                        .overlay(
                            RoundedRectangle(cornerRadius: 4)
                                .strokeBorder(Tokens.line, lineWidth: 1)
                        )
                }
                if active {
                    Circle()
                        .fill(accent.main)
                        .frame(width: 8, height: 8)
                        .shadow(color: accent.main, radius: 5)
                }
            }
            .padding(.horizontal, 16).padding(.vertical, 14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(active ? Color.white.opacity(0.04) : Color.clear)
            .overlay(alignment: .leading) {
                Rectangle().fill(active ? accent.main : .clear).frame(width: 2)
            }
            .opacity(v.installed ? 1.0 : 0.45)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(!v.installed)
        .accessibilityLabel("Voce \(v.display), \(accessibilityGender(v.gender)), \(v.lang)")
        .accessibilityHint(v.installed ? "Doppio tap per selezionare" : "Non installata")
        .accessibilityAddTraits(active ? .isSelected : [])
    }
}

private func accessibilityGender(_ g: Gender) -> String {
    switch g {
    case .female: return "femminile"
    case .male:   return "maschile"
    case .unknown: return "voce"
    }
}

// MARK: — STT engine picker (2 cards side-by-side)

public struct SttPicker: View {
    public var engines: [SttEngineInfo]
    @Binding public var selection: String
    public var accent: Accent

    public init(engines: [SttEngineInfo], selection: Binding<String>, accent: Accent) {
        self.engines = engines; self._selection = selection; self.accent = accent
    }

    public var body: some View {
        let cols = Array(repeating: GridItem(.flexible(), spacing: 10), count: 2)
        LazyVGrid(columns: cols, spacing: 10) {
            ForEach(engines) { eng in
                engineCard(eng)
            }
        }
    }

    @ViewBuilder
    private func engineCard(_ eng: SttEngineInfo) -> some View {
        let active = selection == eng.engineId
        Button(action: { if eng.available { selection = eng.engineId } }) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Circle()
                        .fill(active ? accent.main : Tokens.textGhost)
                        .frame(width: 8, height: 8)
                        .shadow(color: active ? accent.main : .clear, radius: 5)
                    Text(eng.name)
                        .font(.serif(17, italic: active))
                        .foregroundStyle(Tokens.text)
                }
                Text(eng.note)
                    .font(.serif(14, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
                if !eng.available, let reason = eng.reason {
                    Text(reason)
                        .font(.mono(11))
                        .foregroundStyle(Tokens.textFaint)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, minHeight: 92, alignment: .topLeading)
            .background(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(active ? Color.white.opacity(0.04) : Color.white.opacity(0.015))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(active ? accent.main : Tokens.line, lineWidth: 1)
            )
            .shadow(color: active ? accent.glow : .clear, radius: 9)
            .opacity(eng.available ? 1.0 : 0.45)
        }
        .buttonStyle(.plain)
        .disabled(!eng.available)
        .accessibilityLabel("Motore vocale \(eng.name)")
        .accessibilityHint(eng.available ? eng.note : (eng.reason ?? "Non disponibile"))
        .accessibilityAddTraits(active ? .isSelected : [])
    }
}

// MARK: — Voice-command editable row

public struct CommandRow: View {
    @Binding public var cmd: VoiceCommand
    /// Accent of the parent surface — used to tint the user-added
    /// (custom) chips so they read as the user's contribution and stay
    /// visually distinct from the per-language defaults next to them.
    public var accent: Accent
    /// Interface-language code (`"it"`, `"en"`) used to look up which
    /// triggers are defaults for this row. Passed in (rather than read
    /// from `@AppStorage`) so the SettingsView's edit handlers can use
    /// the same value for validation.
    public var interfaceLang: String
    /// Given a candidate trigger, returns the label of another command
    /// that already owns it (nil = free to use). Injected by the parent
    /// so we can validate across the full voice-commands set without the
    /// row having to own the whole list. Defaults of OTHER commands are
    /// included in the conflict set by the caller.
    public var isTakenByOther: (String) -> String?

    @State private var draft: String = ""
    @State private var validationMsg: String? = nil

    public init(cmd: Binding<VoiceCommand>,
                accent: Accent,
                interfaceLang: String,
                isTakenByOther: @escaping (String) -> String? = { _ in nil }) {
        self._cmd = cmd
        self.accent = accent
        self.interfaceLang = interfaceLang
        self.isTakenByOther = isTakenByOther
    }

    public var body: some View {
        HStack(alignment: .top, spacing: 20) {
            VStack(alignment: .leading, spacing: 3) {
                // Display label is the localized one for the current
                // interface language, keyed by action. We fall back to
                // `cmd.label` (whatever the host stored, typically
                // Italian) only if the .strings file is missing the
                // key — a soft signal so a forgotten translation
                // shows the old hardcoded text instead of a raw key.
                Text(localizedLabel)
                    .font(.serif(16))
                    .foregroundStyle(Tokens.text)
                Text(cmd.action)
                    .font(.mono(10))
                    .tracking(0.3)
                    .foregroundStyle(Tokens.textFaint)
            }
            .frame(width: 200, alignment: .leading)
            .padding(.top, 6)

            VStack(alignment: .leading, spacing: 4) {
                // Per-language defaults first (locked, dim) → user-added
                // customs after (accent-tinted, with × to remove). The
                // add-trigger tail input always appends to customs.
                FlowLayout(spacing: 6) {
                    ForEach(defaults, id: \.self) { trigger in
                        Chip(text: trigger, style: .locked)
                            .help(T("settings.commands.chip.default-tooltip"))
                    }
                    ForEach(Array(customs.enumerated()), id: \.offset) { _, trigger in
                        Chip(text: trigger,
                             style: .removable(accent: accent,
                                               onRemove: { removeCustom(trigger) }))
                            .help(T("settings.commands.chip.custom-tooltip"))
                    }
                    addTriggerInput
                }
                if let msg = validationMsg {
                    Text(msg)
                        .font(.mono(10))
                        .foregroundStyle(Color(red: 0.93, green: 0.55, blue: 0.55))
                        .transition(.opacity)
                }
            }
            // Clear the hint the moment the user moves the mouse over the
            // row or clicks anywhere — "non invasiva" in the brief sense.
            .onContinuousHover { _ in
                if validationMsg != nil { validationMsg = nil }
            }
            .onTapGesture { validationMsg = nil }
        }
        .padding(.vertical, 14)
    }

    /// Localized label for the row, derived from the action id. Falls
    /// back to `cmd.label` (the host-provided string) if the
    /// `settings.commands.action.<action>` key is missing from the
    /// active .lproj — keeps the row readable while flagging an
    /// untranslated action via a stale Italian label.
    private var localizedLabel: String {
        let key = "settings.commands.action.\(cmd.action)"
        let translated = T(key)
        return translated == key ? cmd.label : translated
    }

    /// Defaults the picker should display as locked chips, derived
    /// each render from the `(language, action)` table.
    private var defaults: [String] {
        VoiceCommandDefaults.triggers(for: interfaceLang, action: cmd.action)
    }

    /// Triggers stored in `cmd.triggers` that aren't part of *any*
    /// language's default set for this action. Subtracting the union
    /// (rather than just the current language) prevents words from
    /// the previous interface language from popping up as "user
    /// custom" after a language switch — they stay in the TOML so
    /// the resolver keeps recognizing them, the UI just doesn't
    /// misattribute them. Lowercased compare to match the resolver's
    /// normalization.
    private var customs: [String] {
        let blessed = VoiceCommandDefaults.allLanguageTriggers(action: cmd.action)
        return cmd.triggers.filter { !blessed.contains($0.lowercased()) }
    }

    private var addTriggerInput: some View {
        TextField(T("settings.commands.chip.add-placeholder"), text: $draft)
            .textFieldStyle(.plain)
            .font(.serif(14, italic: true))
            .foregroundStyle(Tokens.text)
            .padding(.horizontal, 10).padding(.vertical, 5)
            .frame(width: 120)
            .background(Color.clear)
            .overlay(
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(validationMsg == nil ? Tokens.textGhost
                                  : Color(red: 0.93, green: 0.55, blue: 0.55),
                                  style: StrokeStyle(lineWidth: 1, dash: [3, 3]))
            )
            .onSubmit(submitDraft)
    }

    private func submitDraft() {
        let v = draft.trimmingCharacters(in: .whitespaces).lowercased()
        guard !v.isEmpty else { draft = ""; return }

        // Adding a word that's already a default for THIS command in
        // any supported language is a silent no-op — the user sees
        // it sitting in the locked group when the relevant interface
        // language is active. (Add it once, get it everywhere.)
        if VoiceCommandDefaults.allLanguageTriggers(action: cmd.action)
            .contains(v) {
            draft = ""
            return
        }

        // Already in the customs of this command — silent no-op.
        if cmd.triggers.contains(where: { $0.lowercased() == v }) {
            draft = ""
            return
        }

        // Owned by another command — explain who. The parent's lookup
        // also covers other commands' DEFAULTS, so this catches both
        // "you're trying to add a custom that another command's
        // user-added trigger already owns" and "…that another
        // command's default already owns".
        if let other = isTakenByOther(v) {
            withAnimation(.easeOut(duration: 0.15)) {
                validationMsg = String(
                    format: T("settings.commands.chip.conflict"), v, other
                )
            }
            return
        }

        cmd.triggers.append(v)
        draft = ""
        validationMsg = nil
    }

    private func removeCustom(_ trigger: String) {
        let lc = trigger.lowercased()
        cmd.triggers.removeAll { $0.lowercased() == lc }
    }
}

// MARK: — FlowLayout (macOS 14 has native Layout protocol, use it)

public struct FlowLayout: Layout {
    public var spacing: CGFloat = 6

    public init(spacing: CGFloat = 6) { self.spacing = spacing }

    public func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews,
                             cache: inout ()) -> CGSize {
        let maxW = proposal.width ?? .infinity
        var lineW: CGFloat = 0
        var totalH: CGFloat = 0
        var lineH: CGFloat = 0
        var widestLine: CGFloat = 0
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if lineW + sz.width > maxW, lineW > 0 {
                totalH += lineH + spacing
                widestLine = max(widestLine, lineW - spacing)
                lineW = 0; lineH = 0
            }
            lineW += sz.width + spacing
            lineH = max(lineH, sz.height)
        }
        totalH += lineH
        widestLine = max(widestLine, lineW - spacing)
        return CGSize(width: widestLine, height: totalH)
    }

    public func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize,
                              subviews: Subviews, cache: inout ()) {
        let maxX = bounds.maxX
        var x = bounds.minX
        var y = bounds.minY
        var lineH: CGFloat = 0
        for s in subviews {
            let sz = s.sizeThatFits(.unspecified)
            if x + sz.width > maxX, x > bounds.minX {
                x = bounds.minX
                y += lineH + spacing
                lineH = 0
            }
            s.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(sz))
            x += sz.width + spacing
            lineH = max(lineH, sz.height)
        }
    }
}

// MARK: — Path row (Library, Cache dir)

public struct PathRow: View {
    public var label: String
    public var value: String
    public var tag: String
    public var action: String?
    public var accent: Accent
    /// Invoked when the user taps "sfoglia". Nil means disabled.
    public var onBrowse: (() -> Void)?
    /// Invoked when the user taps the danger action (e.g. "svuota"). Nil = no-op.
    public var onAction: (() -> Void)?
    /// Fired when the user types a new value in-line. Nil = read-only path.
    public var onRevealInFinder: (() -> Void)?

    public init(label: String, value: String, tag: String, action: String?,
                accent: Accent,
                onBrowse: (() -> Void)? = nil,
                onAction: (() -> Void)? = nil,
                onRevealInFinder: (() -> Void)? = nil) {
        self.label = label; self.value = value; self.tag = tag
        self.action = action; self.accent = accent
        self.onBrowse = onBrowse; self.onAction = onAction
        self.onRevealInFinder = onRevealInFinder
    }

    public var body: some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                Text(label.uppercased())
                    .font(.sans(12))
                    .tracking(0.5)
                    .foregroundStyle(Tokens.textDim)
                Text(tag)
                    .font(.mono(10))
                    .foregroundStyle(Tokens.textFaint)
            }
            .frame(width: 140, alignment: .leading)
            Text(value)
                .font(.mono(13))
                .foregroundStyle(Tokens.text)
                .lineLimit(1)
                .truncationMode(.middle)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 6) {
                if let rev = onRevealInFinder {
                    IconBtn(label: "mostra", action: rev)
                }
                if let browse = onBrowse {
                    IconBtn(label: "sfoglia", action: browse)
                }
                if let a = action {
                    IconBtn(label: a, danger: true, action: onAction ?? {})
                }
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.white.opacity(0.02))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }
}

// MARK: — Installations online banner

public struct OnlineBanner: View {
    public var accent: Accent

    public init(accent: Accent) { self.accent = accent }

    public var body: some View {
        HStack(alignment: .center, spacing: 12) {
            ZStack {
                Circle()
                    .fill(Color(hex: 0xEFE5CF, opacity: 0.06))
                Circle().strokeBorder(Tokens.textGhost, lineWidth: 1)
                Image(systemName: "globe")
                    .font(.system(size: 13, weight: .regular))
                    .foregroundStyle(accent.main)
            }
            .frame(width: 28, height: 28)
            Text(attributedBannerText)
                .font(.serif(15, italic: true))
                .foregroundStyle(Tokens.textDim)
                .lineSpacing(3)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.horizontal, 16).padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(accent.main.opacity(0.06))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(accent.main.opacity(0.2), lineWidth: 1)
        )
    }

    private var attributedBannerText: AttributedString {
        var s = AttributedString(
            "Marginalia funziona completamente offline. Questa è l'unica sezione che effettua chiamate di rete — i download vengono da "
        )
        var hf = AttributedString("huggingface.co")
        hf.foregroundColor = Tokens.text
        let amp = AttributedString(" e ")
        var gh = AttributedString("github.com")
        gh.foregroundColor = Tokens.text
        let dot = AttributedString(".")
        s.append(hf); s.append(amp); s.append(gh); s.append(dot)
        return s
    }
}

// MARK: — Install row (per-asset)

public struct InstallRow: View {
    public var item: InstallableAsset
    public var accent: Accent
    /// Transient state from the host's `inflightDownloads` map. `nil` =
    /// not inflight (uses the row's static `item.installed` flag instead).
    public var state: InstallUiState?
    public var onInstall: (String) -> Void
    public var onUninstall: (String) -> Void

    public init(
        item: InstallableAsset,
        accent: Accent,
        state: InstallUiState? = nil,
        onInstall: @escaping (String) -> Void = { _ in },
        onUninstall: @escaping (String) -> Void = { _ in }
    ) {
        self.item = item; self.accent = accent
        self.state = state
        self.onInstall = onInstall
        self.onUninstall = onUninstall
    }

    public var body: some View {
        HStack(spacing: 14) {
            statusIcon
            VStack(alignment: .leading, spacing: 2) {
                Text(item.label)
                    .font(.serif(15, italic: isInstalled))
                    .foregroundStyle(Tokens.text)
                Text(secondaryLine)
                    .font(.mono(10))
                    .tracking(0.3)
                    .foregroundStyle(secondaryColor)
            }
            Spacer()
            trailingControl
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
        .background(Color.white.opacity(0.015))
    }

    private var isInstalled: Bool {
        if item.installed { return true }
        return state == .installed
    }

    private var isInflight: Bool {
        state?.isInflight == true
    }

    private var secondaryLine: String {
        switch state {
        case .queued?:
            return "\(item.size) · \(T("settings.install.state.queued"))"
        case .downloading(let f)?:
            if let f = f {
                return "\(item.size) · \(Int(f * 100))%"
            }
            return "\(item.size) · \(T("settings.install.state.downloading"))"
        case .failed(let m)?:
            let detail = m.isEmpty ? T("settings.install.state.unavailable") : m
            return String(format: T("settings.install.state.error"), detail)
        default:
            let stateLabel = isInstalled
                ? T("settings.install.state.installed")
                : T("settings.install.state.not_installed")
            return "\(item.size) · \(stateLabel)"
        }
    }

    private var secondaryColor: Color {
        if case .failed = state { return .red.opacity(0.85) }
        return Tokens.textFaint
    }

    @ViewBuilder
    private var trailingControl: some View {
        if isInflight {
            // Linear bar when we have a percentage, circular indeterminate
            // otherwise (queued state or first downloading frame before
            // hf-hub init()). 80 pt wide so it doesn't bulk up the row.
            if case .downloading(let f?)? = state {
                ProgressView(value: f, total: 1.0)
                    .progressViewStyle(.linear)
                    .tint(accent.main)
                    .frame(width: 80)
            } else {
                ProgressView()
                    .progressViewStyle(.circular)
                    .controlSize(.small)
                    .tint(accent.main)
            }
        } else if isInstalled, item.removable {
            IconBtn(label: "rimuovi", danger: true) { onUninstall(item.id) }
        } else if !isInstalled {
            let label: String = (state.map {
                if case .failed = $0 { return "riprova" } else { return "installa" }
            }) ?? "installa"
            Button(action: { onInstall(item.id) }) {
                Text(label)
                    .font(.sans(12, weight: .medium))
                    .foregroundStyle(accent.main)
                    .padding(.horizontal, 12).padding(.vertical, 5)
                    .background(
                        RoundedRectangle(cornerRadius: 6)
                            .fill(accent.main.opacity(0.1))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .strokeBorder(accent.main.opacity(0.45), lineWidth: 1)
                    )
            }
            .buttonStyle(.plain)
        }
    }

    @ViewBuilder
    private var statusIcon: some View {
        let isVoice = item.id.hasPrefix("voice")
        ZStack {
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(iconFill)
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(iconStroke, lineWidth: 1)
            if case .failed = state {
                Image(systemName: "exclamationmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.red.opacity(0.85))
            } else if isInstalled {
                Image(systemName: "checkmark")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(accent.main)
            } else if isVoice {
                GenderIcon(gender: .unknown, color: Tokens.textFaint)
                    .scaleEffect(0.55)
            } else {
                Image(systemName: "arrow.down")
                    .font(.system(size: 10))
                    .foregroundStyle(Tokens.textFaint)
            }
        }
        .frame(width: 24, height: 24)
    }

    private var iconFill: Color {
        if case .failed = state { return Color.red.opacity(0.08) }
        return isInstalled ? accent.main.opacity(0.15) : Color.white.opacity(0.03)
    }

    private var iconStroke: Color {
        if case .failed = state { return Color.red.opacity(0.4) }
        return isInstalled ? accent.main.opacity(0.35) : Tokens.line
    }
}

// MARK: — Diagnostics table

public struct DiagTable: View {
    public var spec: ProviderSpec
    public var lastReport: ApplyReport?
    public var doctorJson: String

    public init(spec: ProviderSpec, lastReport: ApplyReport? = nil,
                doctorJson: String = "") {
        self.spec = spec; self.lastReport = lastReport
        self.doctorJson = doctorJson
    }

    public var body: some View {
        let rows = buildRows()
        VStack(spacing: 0) {
            ForEach(Array(rows.enumerated()), id: \.offset) { idx, pair in
                HStack(alignment: .top, spacing: 0) {
                    Text(pair.0.uppercased())
                        .font(.sans(12))
                        .tracking(0.5)
                        .foregroundStyle(Tokens.textDim)
                        .frame(width: 180, alignment: .leading)
                        .padding(.top, 2)
                    Text(pair.1)
                        .font(.mono(12))
                        .foregroundStyle(Tokens.text)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                }
                .padding(.horizontal, 16).padding(.vertical, 10)
                .background(idx % 2 == 0 ? Color.white.opacity(0.015) : Color.clear)
                if idx < rows.count - 1 {
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

    private func buildRows() -> [(String, String)] {
        var rows: [(String, String)] = []
        // Current user selection (provider spec is authoritative for these).
        rows.append(("TTS voice",    spec.voice))
        rows.append(("TTS backend",  spec.ttsBackend))
        rows.append(("STT engine",   spec.sttEngine))
        rows.append(("STT language", spec.language))

        // Parse the doctor JSON and flatten one level deep: each top-level
        // key becomes a row "section · subkey" → value. Falls through silently
        // if the string isn't valid JSON (mock/empty case).
        if let data = doctorJson.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for key in obj.keys.sorted() {
                if let sub = obj[key] as? [String: Any] {
                    for subKey in sub.keys.sorted() {
                        rows.append(("\(key) · \(subKey)",
                                     renderValue(sub[subKey])))
                    }
                } else {
                    rows.append((key, renderValue(obj[key])))
                }
            }
        }

        if let rep = lastReport {
            rows.append(("Ultimo apply",
                         "\(rep.elapsedMs) ms · stt_swapped=\(rep.sttSwapped)"))
        }
        rows.append(("Versione app", "Marginalia 0.9.x (beta)"))
        return rows
    }

    private func renderValue(_ v: Any?) -> String {
        switch v {
        case let b as Bool:       return b ? "true" : "false"
        case let n as NSNumber:   return n.stringValue
        case let s as String:     return s
        case let a as [Any]:      return a.map { "\($0)" }.joined(separator: ", ")
        case .none:               return "—"
        default:                  return "\(v!)"
        }
    }
}
