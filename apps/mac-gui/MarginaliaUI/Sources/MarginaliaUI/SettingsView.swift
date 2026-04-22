import SwiftUI

/// Settings page — mirrors `settings-view.jsx` one-for-one.
///
/// Pattern: staged local state + Apply. The top bar's Apply button is only
/// enabled when the local draft differs from the last-saved spec. When the
/// change requires respawning a provider (STT engine or language), the
/// button's accompanying hint reads "riavvia motore".
public struct SettingsView<Host: MarginaliaHost>: View {

    @ObservedObject private var hostContainer: HostContainer<Host>
    @Binding private var accent: Accent
    public var onClose: () -> Void

    @State private var draft: ProviderSpec
    @State private var savedSpec: ProviderSpec
    @State private var draftCommands: [VoiceCommand]
    @State private var draftChunkChars: Int
    @State private var draftSttDebug: Bool
    @State private var applying: Bool = false
    @State private var lastReport: ApplyReport?
    @State private var errorMessage: String?
    @State private var section: String = "voice"
    @State private var previewing: Bool = false
    /// True during a programmatic scroll triggered by the user clicking
    /// a sub-nav entry. We suppress the preference-driven section update
    /// while this is set, otherwise mid-animation offsets snap `section`
    /// back to wherever the scroll is passing through.
    @State private var programmaticScroll: Bool = false

    // Editable STT tuning values, persisted locally until the FFI
    // `save_config` signature is extended to round-trip them.
    @AppStorage("com.gibbio.marginalia.sttCmdSilence")  private var cmdSilence:  Double = 0.8
    @AppStorage("com.gibbio.marginalia.sttCmdMax")      private var cmdMax:      Double = 4.0
    @AppStorage("com.gibbio.marginalia.sttDictSilence") private var dictSilence: Double = 1.5
    @AppStorage("com.gibbio.marginalia.sttDictMax")     private var dictMax:     Double = 60.0
    // Interface language override — apply() also sets AppleLanguages so the
    // next launch picks up the chosen .lproj bundle.
    @AppStorage(InterfaceLanguage.storageKey) private var interfaceLang: String = "it"
    @State private var languageChangeHint: Bool = false
    @AppStorage(AppTheme.storageKey) private var themeId: String = AppTheme.default.id
    /// Persisted hue for the "Personalizzata" theme. Only read when the
    /// active themeId is `AppTheme.customId`, but the storage is always
    /// available so the user's choice survives theme switches.
    @AppStorage(AppTheme.customHueKey) private var customHue: Double = 30

    // Wrap the host in an ObservableObject box so SwiftUI observes its
    // @Published changes in a protocol-generic way.
    @MainActor
    private final class HostContainer<H: MarginaliaHost>: ObservableObject {
        let host: H
        init(_ host: H) { self.host = host }
    }

    public init(host: Host, accent: Binding<Accent>, onClose: @escaping () -> Void = {}) {
        self.hostContainer = HostContainer(host)
        self._accent = accent
        self.onClose = onClose
        let spec = host.currentSpec
        _draft = State(initialValue: spec)
        _savedSpec = State(initialValue: spec)
        _draftCommands = State(initialValue: host.voiceCommands)
        _draftChunkChars = State(initialValue: host.chunkTargetChars)
        _draftSttDebug = State(initialValue: host.sttDebug)
    }

    private var host: Host { hostContainer.host }
    private var dirty: Bool { draft != savedSpec || draftChunkChars != host.chunkTargetChars }
    private var willSpawn: Bool {
        draft.sttEngine != savedSpec.sttEngine || draft.language != savedSpec.language
    }

    public var body: some View {
        VStack(spacing: 0) {
            topBar
            Divider().frame(height: 1).overlay(Tokens.line)
            HStack(spacing: 0) {
                subNav
                Divider().frame(width: 1).overlay(Tokens.line)
                content
            }
        }
        .background(Tokens.bg)
    }

    // MARK: Top bar

    private var topBar: some View {
        HStack {
            HStack(spacing: 14) {
                Button(action: onClose) {
                    HStack(spacing: 6) {
                        Image(systemName: "chevron.left")
                            .font(.system(size: 11, weight: .semibold))
                        Text("torna alla lettura")
                    }
                    .font(.sans(12))
                    .foregroundStyle(Tokens.textDim)
                    .padding(.leading, 6).padding(.trailing, 10).padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                Rectangle().fill(Tokens.line).frame(width: 1, height: 16)
                Text("Impostazioni")
                    .font(.serif(20, italic: true))
                    .foregroundStyle(Tokens.text)
            }
            Spacer()
            HStack(spacing: 14) {
                if let err = errorMessage {
                    Text(err)
                        .font(.mono(10))
                        .foregroundStyle(.red.opacity(0.8))
                } else if dirty {
                    Text("modifiche in sospeso\(willSpawn ? " · riavvia motore" : "")")
                        .font(.mono(10))
                        .tracking(0.5)
                        .foregroundStyle(accent.main)
                }
                ApplyButton(
                    dirty: dirty,
                    applying: applying,
                    accent: accent,
                    action: applyTapped
                )
            }
        }
        .padding(.horizontal, 24)
        .frame(height: 52)
    }

    private func applyTapped() {
        guard dirty, !applying else { return }
        applying = true
        errorMessage = nil
        Task {
            do {
                let report = try await host.apply(spec: draft)
                host.saveAudioPrefs(chunkTargetChars: draftChunkChars, sttDebug: draftSttDebug)
                if draftCommands != host.voiceCommands {
                    host.saveVoiceCommands(draftCommands)
                }
                await MainActor.run {
                    savedSpec = draft
                    lastReport = report
                    applying = false
                }
            } catch {
                await MainActor.run {
                    errorMessage = "\(error)"
                    applying = false
                }
            }
        }
    }

    // MARK: Sub-nav

    private var subNav: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(SettingsNavEntry.all, id: \.key) { entry in
                let active = section == entry.key
                Button(action: { section = entry.key }) {
                    Text(entry.label)
                        .font(.serif(14, italic: active))
                        .foregroundStyle(active ? Tokens.text : Tokens.textDim)
                        .padding(.horizontal, 20).padding(.vertical, 8)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(active ? Color.white.opacity(0.04) : Color.clear)
                        .overlay(alignment: .leading) {
                            Rectangle()
                                .fill(active ? accent.main : Color.clear)
                                .frame(width: 2)
                        }
                }
                .buttonStyle(.plain)
            }
            Divider().frame(height: 1).overlay(Tokens.line).padding(.horizontal, 20).padding(.vertical, 14)
            Text("FILE")
                .font(.mono(9))
                .tracking(1.2)
                .foregroundStyle(Tokens.textFaint)
                .padding(.horizontal, 20).padding(.bottom, 4)
            Text("~/Library/App Support/\nMarginalia/marginalia.toml")
                .font(.mono(10))
                .foregroundStyle(Tokens.textDim)
                .padding(.horizontal, 20)
                .lineSpacing(2)
            Spacer()
        }
        .padding(.vertical, 18)
        .frame(width: 200)
        .background(Color.black.opacity(0.12))
    }

    // MARK: Content scroller

    private var content: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 56) {
                    sectionAnchor(languageSection, key: "language")
                    sectionAnchor(voiceSection, key: "voice")
                    sectionAnchor(sttSection, key: "stt")
                    sectionAnchor(commandsSection, key: "commands")
                    sectionAnchor(audioSection, key: "audio")
                    sectionAnchor(themeSection, key: "theme")
                    sectionAnchor(installationsSection, key: "installations")
                    sectionAnchor(interfaceLanguageSection, key: "interface")
                    sectionAnchor(diagnosticsSection, key: "diagnostics")
                }
                .padding(.horizontal, 48).padding(.top, 24).padding(.bottom, 80)
                .frame(maxWidth: 720 + 96, alignment: .leading)  // 720 content + padding
            }
            .coordinateSpace(name: "settingsScroll")
            .onPreferenceChange(SectionOffsetsKey.self) { offsets in
                // Ignore scroll-driven updates while we're programmatically
                // scrolling in response to a sub-nav click — otherwise the
                // intermediate offsets would snap `section` back before we
                // reach the target.
                guard !programmaticScroll else { return }
                let threshold: CGFloat = 120
                let candidates = offsets.filter { $0.value <= threshold }
                if let best = candidates.max(by: { $0.value < $1.value }) {
                    if section != best.key { section = best.key }
                }
            }
            .onChange(of: section) { _, new in
                // User clicked a sub-nav entry — scroll programmatically
                // and gate the preference observer for the animation window.
                programmaticScroll = true
                withAnimation(.easeInOut(duration: 0.25)) {
                    proxy.scrollTo(new, anchor: .top)
                }
                Task {
                    // A tick longer than the animation so the final offsets
                    // stabilize before we re-enable the observer.
                    try? await Task.sleep(for: .milliseconds(450))
                    await MainActor.run { programmaticScroll = false }
                }
            }
        }
    }

    @ViewBuilder
    private func sectionAnchor<V: View>(_ content: V, key: String) -> some View {
        content
            .id(key)
            .background(
                GeometryReader { g in
                    Color.clear.preference(
                        key: SectionOffsetsKey.self,
                        value: [key: g.frame(in: .named("settingsScroll")).minY]
                    )
                }
            )
    }

    // MARK: — Sections —

    private var languageSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionHeader(
                kicker: "001", title: "Lingua",
                sub: "determina le voci disponibili e la lingua di default del riconoscimento.",
                info: "La lingua scelta filtra le voci del TTS mostrate qui sotto e imposta il locale di default del riconoscimento vocale. La lingua dell'interfaccia si cambia in fondo a questa pagina."
            )
            LangPicker(
                languages: host.languages,
                selection: draft.language,
                accent: accent,
                onChange: onLangChange
            )
            if host.voices.filter({ $0.lang == draft.language && $0.installed }).isEmpty {
                HintCard(accent: accent) {
                    HStack(spacing: 4) {
                        Text("Nessuna voce installata per questa lingua.")
                        Text("Installa voci…")
                            .foregroundStyle(accent.main)
                            .underline()
                    }
                }
                .padding(.top, 14)
            }
        }
    }

    private func onLangChange(_ code: String) {
        draft.language = code
        let voices = host.voices.filter { $0.lang == code && $0.installed }
        if !voices.contains(where: { $0.id == draft.voice }) {
            draft.voice = voices.first?.id ?? ""
        }
    }

    private var voiceSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionHeader(
                kicker: "002", title: "Voce",
                sub: "la voce usata per leggerti il testo ad alta voce.",
                info: "Ogni voce è un piccolo modello neurale (≈500 KB) scaricato in locale. Il picker mostra solo le voci installate per la lingua corrente; quelle non installate sono grigie."
            )
            VoicePicker(
                voices: host.voices.filter { $0.lang == draft.language },
                selection: $draft.voice,
                accent: accent
            )
            voicePreview.padding(.top, 18)
        }
    }

    private var voicePreview: some View {
        HStack(spacing: 16) {
            Button(action: previewVoice) {
                PlayButton(accent: accent.main, glow: accent.glow)
                    .opacity(previewing ? 0.5 : 1.0)
            }
            .buttonStyle(.plain)
            .disabled(previewing)
            .accessibilityLabel("Ascolta anteprima voce")
            VStack(alignment: .leading, spacing: 3) {
                Text("ANTEPRIMA")
                    .font(.mono(10))
                    .tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                Text("\u{201C}Il tempo, nell'alta montagna, non è il tempo della pianura.\u{201D}")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.text)
                    .lineSpacing(3)
            }
            Spacer()
            Waveform(count: 30, accent: accent.main, dim: Tokens.textFaint)
        }
        .padding(14).padding(.horizontal, 2)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.white.opacity(0.02))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }

    private func previewVoice() {
        previewing = true
        let sample = "Il tempo, nell'alta montagna, non è il tempo della pianura."
        Task {
            defer { Task { @MainActor in previewing = false } }
            do {
                let path = try await host.synthesizePreview(text: sample, voice: draft.voice)
                guard !path.isEmpty else { return }
                await MainActor.run { Self.playAudio(at: path) }
            } catch {
                await MainActor.run {
                    errorMessage = "Anteprima fallita: \(error)"
                }
            }
        }
    }

    private static func playAudio(at path: String) {
        #if canImport(AppKit)
        PreviewSoundCache.play(path: path)
        #endif
    }

    private var sttSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionHeader(
                kicker: "003", title: "Riconoscimento vocale",
                sub: "ascolta i tuoi comandi e registra le note dettate.",
                info: "Apple Speech è il riconoscitore di sistema (basso costo, alta qualità, richiede la dettatura macOS attiva). Whisper è un modello ggml locale da 465 MB, completamente offline. Il cambio richiede di riavviare l'helper — qualche centinaio di ms."
            )
            SttPicker(
                engines: host.sttEngines,
                selection: $draft.sttEngine,
                accent: accent
            )
            miniStatGrid.padding(.top, 16)
            AccentCheckbox(checked: $draftSttDebug,
                           label: "Mostra trascrizione grezza nel log",
                           accent: accent)
                .padding(.top, 16)
            if draft.sttEngine == "apple" {
                HintCard(accent: accent) {
                    HStack(spacing: 10) {
                        Text("Apple Speech richiede la dettatura macOS.")
                        Text("Apri Impostazioni di sistema →")
                            .foregroundStyle(accent.main)
                            .underline()
                    }
                }
                .padding(.top, 16)
                .onTapGesture { openAppleSpeechSettings() }
            }
            if draft.sttEngine == "whisper" {
                whisperModelRow.padding(.top, 16)
            }
        }
    }

    @ViewBuilder
    private var whisperModelRow: some View {
        let path = host.whisperModelPath ?? ""
        let url  = path.isEmpty ? nil : URL(fileURLWithPath: path)
        let installed = url.map { FileManager.default.fileExists(atPath: $0.path) } ?? false

        PathRow(
            label: "Modello Whisper",
            value: path.isEmpty ? "non configurato" : path,
            tag: installed ? "ggml · offline" : "mancante · make bootstrap-whisper",
            action: nil,
            accent: accent,
            onBrowse: nil,  // picker+save requires an FFI setter; see plan.
            onRevealInFinder: url.flatMap { u in
                installed ? { revealInFinder(u) } : nil
            }
        )
    }

    private func revealInFinder(_ url: URL) {
        #if canImport(AppKit)
        NSWorkspace.shared.activateFileViewerSelecting([url])
        #endif
    }

    private var miniStatGrid: some View {
        let cols = [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)]
        return LazyVGrid(columns: cols, spacing: 12) {
            EditableStat(label: "Comandi · timeout silenzio",
                         unit: "s",
                         value: $cmdSilence,
                         range: 0.3...3.0,
                         step: 0.1)
            EditableStat(label: "Comandi · durata max",
                         unit: "s",
                         value: $cmdMax,
                         range: 1.0...30.0,
                         step: 0.5)
            EditableStat(label: "Dettatura · timeout silenzio",
                         unit: "s",
                         value: $dictSilence,
                         range: 0.5...5.0,
                         step: 0.1)
            EditableStat(label: "Dettatura · durata max",
                         unit: "s",
                         value: $dictMax,
                         range: 5.0...300.0,
                         step: 1,
                         format: "%.0f")
        }
    }

    private func openAppleSpeechSettings() {
        #if canImport(AppKit)
        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.keyboard") {
            NSWorkspace.shared.open(url)
        }
        #endif
    }

    private var commandsSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionHeader(
                kicker: "004", title: "Comandi vocali",
                sub: "parole che, se pronunciate, attivano un'azione. le modifiche si salvano al volo.",
                info: "Ogni azione ha uno o più sinonimi. Le parole possono essere in qualsiasi lingua — non sono tradotte, è l'audio grezzo che conta. Parole già usate da un'altra azione verranno rifiutate per evitare ambiguità."
            )
            VStack(alignment: .leading, spacing: 0) {
                Divider().frame(height: 1).overlay(Tokens.line)
                ForEach($draftCommands) { $cmd in
                    CommandRow(
                        cmd: $cmd,
                        isTakenByOther: { candidate in
                            triggerConflict(candidate: candidate, excluding: cmd.action)
                        }
                    )
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Divider().frame(height: 1).overlay(Tokens.lineSoft)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    /// Look-up used by CommandRow to reject a trigger that's already in
    /// use by a different command. Returns the user-facing label (e.g.
    /// "Metti in pausa") of the conflicting command, or nil if the word
    /// is free.
    private func triggerConflict(candidate raw: String, excluding own: String) -> String? {
        let needle = raw.trimmingCharacters(in: .whitespaces).lowercased()
        guard !needle.isEmpty else { return nil }
        for cmd in draftCommands where cmd.action != own {
            if cmd.triggers.contains(where: { $0.lowercased() == needle }) {
                return cmd.label
            }
        }
        return nil
    }

    private var audioSection: some View {
        VStack(alignment: .leading, spacing: 22) {
            SectionHeader(
                kicker: "005", title: "Preparazione testo",
                sub: "dimensione dei chunk, percorsi di libreria e cache audio.",
                info: "Il chunk è l'unità minima di sintesi e navigazione vocale. Chunk piccoli = risposta rapida ai comandi e salti fini, ma più frammentazione. Il cambio richiede di re-importare i documenti esistenti. La cache audio contiene i FLAC sintetizzati; cancellarla non perde lavoro, solo tempo di rigenerazione."
            )
            volumeSlider
            chunkSizeSlider
            PathRow(label: "Libreria",
                    value: ".marginalia/beta.sqlite3",
                    tag: "SQLite", action: nil, accent: accent)
            PathRow(label: "Cache audio",
                    value: ".marginalia/tts-cache",
                    tag: "FLAC · 1 per chunk · \(cacheDirSize())",
                    action: "svuota", accent: accent)
        }
    }

    /// Playback volume slider, 0.0–1.0 (linear). Two-way bound to
    /// `host.volume` so the change reaches rodio immediately. Keyboard
    /// shortcuts (⌘↑/⌘↓) also write here through the menu.
    private var volumeSlider: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Volume")
                    .font(.mono(10)).tracking(1.2)
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                Text("\(Int(host.volume * 100))%")
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .monospacedDigit()
            }
            HStack(spacing: 10) {
                Image(systemName: "speaker.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(Tokens.textFaint)
                Slider(
                    value: Binding(
                        get: { host.volume },
                        set: { host.volume = $0 }
                    ),
                    in: 0.0...1.0
                )
                .tint(accent.main)
                Image(systemName: "speaker.wave.3.fill")
                    .font(.system(size: 10))
                    .foregroundStyle(Tokens.textFaint)
            }
        }
    }

    /// Returns a formatted size string for the TTS cache directory, e.g.
    /// "142 MB" or "0 byte". Walks the dir synchronously — cheap for the
    /// expected order of magnitude (a few thousand FLAC files max).
    private func cacheDirSize() -> String {
        let path = ".marginalia/tts-cache"
        let url = URL(fileURLWithPath: path)
        guard let enumerator = FileManager.default.enumerator(
            at: url,
            includingPropertiesForKeys: [.fileSizeKey, .isRegularFileKey]
        ) else { return "—" }
        var total: Int = 0
        for case let file as URL in enumerator {
            guard let values = try? file.resourceValues(
                forKeys: [.fileSizeKey, .isRegularFileKey]
            ) else { continue }
            if values.isRegularFile == true, let size = values.fileSize {
                total += size
            }
        }
        return ByteCountFormatter.string(fromByteCount: Int64(total),
                                         countStyle: .file)
    }

    private var chunkSizeSlider: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Text("DIMENSIONE CHUNK")
                    .font(.sans(11))
                    .tracking(0.5)
                    .foregroundStyle(Tokens.textDim)
                Text("· \(draftChunkChars) caratteri")
                    .font(.sans(11))
                    .foregroundStyle(Tokens.textFaint)
            }
            Slider(
                value: Binding(
                    get: { Double(draftChunkChars) },
                    set: { draftChunkChars = Int($0 / 25) * 25 }
                ),
                in: 100...300, step: 25
            )
            .tint(accent.main)
            HStack {
                Text("100 · più navigabile, sintesi rapida")
                Spacer()
                Text("300 · ascolto continuo")
            }
            .font(.mono(9))
            .foregroundStyle(Tokens.textFaint)
            if draftChunkChars != host.chunkTargetChars {
                Text("⚠ il cambio richiederà di re-importare i documenti esistenti.")
                    .font(.serif(12, italic: true))
                    .foregroundStyle(accent.main)
                    .padding(.top, 4)
            }
        }
    }

    private var installationsSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "006", title: "Installazioni",
                sub: "modelli e voci sul disco. questa è l'unica sezione che scarica dalla rete.",
                info: "Marginalia funziona offline. Solo qui, su esplicita azione tua, l'app può contattare huggingface.co e github.com per scaricare modelli mancanti. Tutti gli asset sono Apache-2.0 o MIT."
            )
            OnlineBanner(accent: accent)

            // Non-voice assets flat at the top — one install per user
            // per lifetime, no grouping needed.
            let coreAssets = host.installations.filter { $0.category != "voice" }
            if !coreAssets.isEmpty {
                installRowStack(coreAssets)
            }

            // Voices grouped by BCP-47 language with a disclosure + an
            // "installa tutte" bulk button. Keeps the list navigable
            // when the catalog grows to dozens of voices.
            ForEach(voiceGroups, id: \.language) { group in
                voiceLanguageSection(group)
            }
        }
        .onAppear { Task { await host.refreshInstallations() } }
    }

    /// Voices grouped by language, system-current first then alphabetical.
    private var voiceGroups: [(language: String, display: String, assets: [InstallableAsset])] {
        let voices = host.installations.filter { $0.category == "voice" }
        let byLang = Dictionary(grouping: voices, by: { $0.language ?? "und" })
        let sysLang = Locale.current.language.languageCode?.identifier ?? "en"
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

    private static func languageDisplay(_ bcp47: String) -> String {
        let code = String(bcp47.prefix(2)).lowercased()
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

    @ViewBuilder
    private func voiceLanguageSection(
        _ group: (language: String, display: String, assets: [InstallableAsset])
    ) -> some View {
        let missing = group.assets.filter { !$0.installed }
        DisclosureGroup {
            installRowStack(group.assets)
                .padding(.top, 6)
        } label: {
            HStack {
                Text(group.display)
                    .font(.serif(14, italic: true))
                    .foregroundStyle(Tokens.text)
                Text("(\(group.assets.filter { $0.installed }.count)/\(group.assets.count))")
                    .font(.mono(10))
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                if !missing.isEmpty {
                    Button("installa tutte") {
                        for a in missing { host.installAsset(a.id) }
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
        }
        .padding(.horizontal, 4)
        .tint(Tokens.textFaint)
    }

    @ViewBuilder
    private func installRowStack(_ assets: [InstallableAsset]) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(assets.enumerated()), id: \.element.id) { idx, item in
                InstallRow(
                    item: item,
                    accent: accent,
                    state: host.inflightDownloads[item.id],
                    onInstall: { host.installAsset($0) },
                    onUninstall: { host.uninstallAsset($0) }
                )
                if idx < assets.count - 1 {
                    Divider().frame(height: 1).overlay(Tokens.lineSoft)
                }
            }
        }
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.white.opacity(0.015))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }

    private var themeSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "006", title: "Tema",
                sub: "la palette dell'interfaccia. il cambio è immediato.",
                info: "Cambia la tinta di accento — highlight, player pill, barre audio, hover nelle note. Lo sfondo warm-dark resta invariato: è parte dell'identità di Marginalia."
            )
            HStack(spacing: 12) {
                ForEach(AppTheme.all, id: \.id) { theme in
                    themeCard(theme)
                }
                Spacer()
            }
            if themeId == AppTheme.customId {
                customHueSlider
            }
        }
    }

    /// Hue slider shown only when the "Personalizzata" theme is active.
    /// Writes to `AppTheme.customHueKey` — `AppTheme.accent(for:)` reads
    /// from there on every call, so the live app picks up the change on
    /// the next `onChange(of: themeId)` in MarginaliaWindow.
    @ViewBuilder
    private var customHueSlider: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Tinta accento")
                    .font(.mono(10)).tracking(1.2)
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                Text("\(Int(customHue))°")
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .monospacedDigit()
            }
            Slider(value: $customHue, in: 0...360) { _ in
                // Bump the themeId notification so MarginaliaWindow
                // re-runs `AppTheme.accent(for:)` and we repaint.
                let cur = themeId
                themeId = ""
                themeId = cur
            }
            .tint(Accent(hue: customHue).main)
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color.white.opacity(0.02))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }

    @ViewBuilder
    private func themeCard(_ theme: Theme) -> some View {
        let active = themeId == theme.id
        let previewAccent = Accent(hue: theme.accentHue)
        Button(action: { themeId = theme.id }) {
            VStack(alignment: .leading, spacing: 10) {
                // Preview swatches — a tiny "page fragment" with the
                // theme's accent applied so the user sees the colour
                // on warm-dark, not isolated.
                ZStack {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .fill(Tokens.bg)
                    HStack(spacing: 6) {
                        Circle()
                            .fill(previewAccent.main)
                            .frame(width: 10, height: 10)
                            .shadow(color: previewAccent.glow, radius: 4)
                        HStack(spacing: 1.5) {
                            ForEach(0..<16, id: \.self) { i in
                                Rectangle()
                                    .fill(previewAccent.main)
                                    .frame(width: 1.5, height: 4 + CGFloat(abs(sin(Double(i) * 0.5))) * 10)
                                    .opacity(0.4 + Double(i % 3) * 0.2)
                            }
                        }
                        Spacer()
                        Text("M")
                            .font(.serif(18, italic: true))
                            .foregroundStyle(Tokens.text)
                    }
                    .padding(.horizontal, 12)
                }
                .frame(height: 44)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .strokeBorder(Tokens.line, lineWidth: 1)
                )

                VStack(alignment: .leading, spacing: 2) {
                    Text(theme.display)
                        .font(.serif(17, italic: active))
                        .foregroundStyle(Tokens.text)
                    Text(theme.blurb)
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .padding(14)
            .frame(width: 280, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(active ? Color.white.opacity(0.04) : Color.white.opacity(0.015))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(active ? previewAccent.main : Tokens.line, lineWidth: 1)
            )
            .shadow(color: active ? previewAccent.glow : .clear, radius: 9)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Tema \(theme.display)")
        .accessibilityHint(theme.blurb)
        .accessibilityAddTraits(active ? .isSelected : [])
    }

    private var interfaceLanguageSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "008", title: "Lingua interfaccia",
                sub: "la lingua dei testi dell'app. il cambio richiede di riavviare Marginalia.",
                info: "Diversa dalla lingua di lettura/riconoscimento della sezione Lingua: qui scegli in che lingua l'app stessa ti parla — menu, pulsanti, Settings. Le traduzioni vivono in \(InterfaceLanguage.storageKey).lproj."
            )

            HStack(spacing: 10) {
                ForEach(InterfaceLanguage.Code.allCases, id: \.rawValue) { code in
                    interfaceLangPill(code)
                }
                Spacer()
            }

            if languageChangeHint {
                HintCard(accent: accent) {
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.clockwise.circle")
                            .foregroundStyle(accent.main)
                        Text("Riavvia Marginalia per applicare la nuova lingua.")
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func interfaceLangPill(_ code: InterfaceLanguage.Code) -> some View {
        let active = interfaceLang == code.rawValue
        Button(action: { selectInterfaceLang(code) }) {
            Text(InterfaceLanguage.displayName(code))
                .font(.serif(15, italic: active))
                .foregroundStyle(active ? Tokens.text : Tokens.textDim)
                .padding(.horizontal, 14).padding(.vertical, 8)
                .background(
                    Capsule()
                        .fill(active ? Color.white.opacity(0.05) : Color.white.opacity(0.015))
                )
                .overlay(
                    Capsule()
                        .strokeBorder(active ? accent.main : Tokens.line, lineWidth: 1)
                )
                .shadow(color: active ? accent.glow : .clear, radius: 9)
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Lingua interfaccia \(InterfaceLanguage.displayName(code))")
        .accessibilityAddTraits(active ? .isSelected : [])
    }

    private func selectInterfaceLang(_ code: InterfaceLanguage.Code) {
        guard interfaceLang != code.rawValue else { return }
        InterfaceLanguage.apply(code)
        withAnimation(.easeOut(duration: 0.2)) {
            languageChangeHint = true
        }
    }

    private var diagnosticsSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "007", title: "Diagnostica",
                sub: "informazioni utili per il supporto — sola lettura.",
                info: "Stato vivo dei provider e dei percorsi sul filesystem. Include l'ultimo ApplyReport (tempo impiegato, cosa è stato scambiato). Utile quando qualcosa non funziona — selezionabile per copia-incolla."
            )
            DiagTable(
                spec: draft,
                lastReport: lastReport,
                doctorJson: host.doctorReportJson()
            )
            // Export: bundles the doctor report + recent log lines into a
            // single text file the user can attach to a bug report.
            HStack {
                Button(action: exportDiagnostics) {
                    HStack(spacing: 8) {
                        Image(systemName: "square.and.arrow.up")
                            .font(.system(size: 11, weight: .medium))
                        Text("Esporta log…")
                            .font(.sans(12, weight: .medium))
                    }
                    .foregroundStyle(Tokens.textDim)
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(
                        Capsule().fill(Color.white.opacity(0.03))
                    )
                    .overlay(
                        Capsule().strokeBorder(Tokens.line, lineWidth: 1)
                    )
                }
                .buttonStyle(.plain)
                .help("Salva un .txt con diagnostica e log recenti")
                .accessibilityLabel("Esporta diagnostica e log")

                Button(action: confirmResetPreferences) {
                    HStack(spacing: 8) {
                        Image(systemName: "arrow.counterclockwise")
                            .font(.system(size: 11, weight: .medium))
                        Text("Ripristina predefiniti…")
                            .font(.sans(12, weight: .medium))
                    }
                    .foregroundStyle(Color.red.opacity(0.8))
                    .padding(.horizontal, 14).padding(.vertical, 8)
                    .background(Capsule().fill(Color.red.opacity(0.06)))
                    .overlay(Capsule().strokeBorder(Color.red.opacity(0.3), lineWidth: 1))
                }
                .buttonStyle(.plain)
                .help("Ripristina le preferenze dell'interfaccia (tema, dimensione chunk, debug STT) ai valori predefiniti. I documenti e le note non vengono toccati.")

                Spacer()
            }
        }
    }

    /// Reset UI preferences + reading params to defaults. Does NOT touch
    /// provider spec (could leave the app unusable if defaults aren't
    /// installed) and does NOT delete user content (documents, notes,
    /// bookmarks). Confirmation required — preferences are cheap to
    /// re-set but muscle memory isn't.
    private func confirmResetPreferences() {
        #if canImport(AppKit)
        let alert = NSAlert()
        alert.messageText = "Ripristinare le preferenze?"
        alert.informativeText = "Tema, dimensione dei chunk, debug STT, scorciatoie tornano ai valori predefiniti. Documenti, note e segnalibri NON vengono toccati."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Ripristina")
        alert.addButton(withTitle: "Annulla")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        #endif
        // UI / reading prefs — stored in UserDefaults.
        let ud = UserDefaults.standard
        ud.removeObject(forKey: AppTheme.storageKey)
        ud.removeObject(forKey: AppTheme.customHueKey)
        ud.removeObject(forKey: ReadingPrefs.modeKey)
        ud.removeObject(forKey: ReadingPrefs.scaleKey)
        themeId = AppTheme.default.id
        customHue = 30
        // Runtime prefs — persisted via save_config on Apply.
        draftChunkChars = 300
        draftSttDebug = false
        host.saveAudioPrefs(chunkTargetChars: 300, sttDebug: false)
        host.transientToast = ToastMessage(text: "Preferenze ripristinate.", kind: .info)
    }

    /// Assemble a single text dump (ISO timestamp header, doctor JSON,
    /// recent messages buffer) and write it via NSSavePanel. Diagnostic
    /// data only — no user content.
    private func exportDiagnostics() {
        let iso = ISO8601DateFormatter()
        iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]

        var payload =
        """
        # Marginalia — diagnostic export
        timestamp: \(iso.string(from: Date()))
        provider_spec: \(draft)
        last_apply_report: \(lastReport.map { String(describing: $0) } ?? "—")

        ## Doctor report
        \(host.doctorReportJson())

        ## Recent messages (\(host.messages.count))
        """
        for m in host.messages { payload.append("\n\(m)") }

        #if canImport(AppKit)
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.plainText]
        panel.nameFieldStringValue = "marginalia-diagnostics.txt"
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url {
            do {
                try payload.write(to: url, atomically: true, encoding: .utf8)
                errorMessage = nil
            } catch {
                errorMessage = "Esportazione fallita: \(error.localizedDescription)"
            }
        }
        #endif
    }
}

// MARK: — Top-bar Apply button

private struct ApplyButton: View {
    var dirty: Bool
    var applying: Bool
    var accent: Accent
    var action: () -> Void

    @State private var spin: Double = 0

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                if applying {
                    Circle()
                        .trim(from: 0.1, to: 0.9)
                        .stroke(Tokens.bg.opacity(0.5), style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
                        .frame(width: 10, height: 10)
                        .rotationEffect(.degrees(spin))
                        .onAppear {
                            withAnimation(.linear(duration: 0.7).repeatForever(autoreverses: false)) {
                                spin = 360
                            }
                        }
                }
                Text(applying ? "Applicazione…" : "Applica")
            }
            .font(.sans(12, weight: .medium))
            .foregroundStyle(dirty ? Tokens.bg : Tokens.textFaint)
            .padding(.horizontal, 16).padding(.vertical, 7)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(dirty ? accent.main : Color.white.opacity(0.08))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .strokeBorder(dirty ? accent.main : Tokens.line, lineWidth: 1)
            )
            .shadow(color: dirty ? accent.glow : .clear, radius: 10)
        }
        .buttonStyle(.plain)
        .disabled(!dirty || applying)
    }
}

// MARK: — Scroll tracking

/// Maps each section key (`"language"`, `"voice"`, …) to its current minY
/// in the ScrollView's coordinate space. Used by SettingsView to highlight
/// the sub-nav entry that matches whatever the user has scrolled into view.
struct SectionOffsetsKey: PreferenceKey {
    static let defaultValue: [String: CGFloat] = [:]
    static func reduce(value: inout [String: CGFloat], nextValue: () -> [String: CGFloat]) {
        value.merge(nextValue(), uniquingKeysWith: { _, new in new })
    }
}

// MARK: — Sub-nav entries

private struct SettingsNavEntry {
    let key: String
    let label: String
    static let all: [SettingsNavEntry] = [
        .init(key: "language", label: "Lingua"),
        .init(key: "voice", label: "Voce"),
        .init(key: "stt", label: "Riconoscimento vocale"),
        .init(key: "commands", label: "Comandi vocali"),
        .init(key: "audio", label: "Preparazione testo"),
        .init(key: "theme", label: "Tema"),
        .init(key: "installations", label: "Installazioni"),
        .init(key: "interface", label: "Lingua interfaccia"),
        .init(key: "diagnostics", label: "Diagnostica"),
    ]
}
