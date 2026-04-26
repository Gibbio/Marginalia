import SwiftUI
#if canImport(AVFoundation)
import AVFoundation
#endif

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
    /// AVAudioPlayer kept alive while a preview is playing so the
    /// player isn't deallocated mid-playback (which silently kills
    /// audio output). Rebuilt for each preview tap.
    @State private var previewPlayer: AVAudioPlayer? = nil
    /// True while preview audio is actively coming out of the speakers.
    /// Drives the play↔stop glyph toggle.
    @State private var isPreviewPlaying: Bool = false
    /// Per-bucket peak amplitudes (0…1) sampled from the just-
    /// synthesized WAV. Empty until the first preview completes;
    /// then drives the Waveform widget next to the play button.
    @State private var previewWaveformLevels: [Float] = []
    /// True during a programmatic scroll triggered by the user clicking
    /// a sub-nav entry. We suppress the preference-driven section update
    /// while this is set, otherwise mid-animation offsets snap `section`
    /// back to wherever the scroll is passing through.
    @State private var programmaticScroll: Bool = false
    /// Confirmation sheet for the destructive "svuota cache audio"
    /// action. The body view reads the current cache size at render
    /// time, so the dialog's tagline always shows the up-to-date
    /// number even if the user lingers before confirming.
    @State private var showClearCacheConfirm: Bool = false
    /// Set while `host.clearTtsCache()` is running so the sheet's
    /// confirm button can render a spinner. Brief on a small cache;
    /// for a multi-GB cache the file walk dominates.
    @State private var clearingCache: Bool = false
    /// Confirmation sheet for the **destructive** "svuota cache note"
    /// action. Wipes every voice note (DB + audio file). Two-step gate
    /// so the user can't trip it accidentally — the Conferma button is
    /// disabled until they tick `notesConfirmAcknowledged`.
    @State private var showClearNotesConfirm: Bool = false
    @State private var clearingNotes: Bool = false
    @State private var notesConfirmAcknowledged: Bool = false

    // Editable STT tuning values, persisted locally until the FFI
    // `save_config` signature is extended to round-trip them.
    @AppStorage("com.gibbio.marginalia.sttCmdSilence")  private var cmdSilence:  Double = 0.8
    @AppStorage("com.gibbio.marginalia.sttCmdMax")      private var cmdMax:      Double = 4.0
    @AppStorage("com.gibbio.marginalia.sttDictSilence") private var dictSilence: Double = 1.5
    @AppStorage("com.gibbio.marginalia.sttDictMax")     private var dictMax:     Double = 60.0
    // Interface language override — apply() also sets AppleLanguages so the
    // next launch picks up the chosen .lproj bundle.
    @AppStorage(InterfaceLanguage.storageKey) private var interfaceLang: String = "it"
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
                        Text(T("settings.top.back"))
                    }
                    .font(.sans(12))
                    .foregroundStyle(Tokens.textDim)
                    .padding(.leading, 6).padding(.trailing, 10).padding(.vertical, 6)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                Rectangle().fill(Tokens.line).frame(width: 1, height: 16)
                Text(T("settings.top.title"))
                    .font(.serif(16, italic: true))
                    .foregroundStyle(Tokens.text)
            }
            Spacer()
            HStack(spacing: 14) {
                if let err = errorMessage {
                    Text(err)
                        .font(.mono(10))
                        .foregroundStyle(.red.opacity(0.8))
                } else if dirty {
                    Text(T("settings.top.pending")
                         + (willSpawn ? " · " + T("settings.top.respawn") : ""))
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
        .frame(height: 40)
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
            Text(Self.tildeify(host.configPath))
                .font(.mono(10))
                .foregroundStyle(Tokens.textDim)
                .padding(.horizontal, 20)
                .lineSpacing(2)
                .textSelection(.enabled)
                .help(host.configPath)
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
                kicker: "001",
                title: T("settings.nav.language"),
                sub: T("settings.sub.language"),
                info: T("settings.info.language")
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
                        Text(T("settings.lang.no-voices"))
                        Button(action: { section = "installations" }) {
                            Text(T("settings.lang.install-cta"))
                                .foregroundStyle(accent.main)
                                .underline()
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
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
                kicker: "002",
                title: T("settings.nav.voice"),
                sub: T("settings.sub.voice"),
                info: T("settings.info.voice")
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
            Button(action: togglePreview) {
                ZStack {
                    Circle()
                        .fill(accent.main)
                        .shadow(color: accent.glow, radius: 8)
                    if isPreviewPlaying {
                        // Stop glyph (filled square).
                        Rectangle()
                            .fill(Tokens.bg)
                            .frame(width: 12, height: 12)
                    } else if previewing {
                        // Synth in flight — small spinner.
                        ProgressView()
                            .progressViewStyle(.circular)
                            .controlSize(.small)
                            .tint(Tokens.bg)
                    } else {
                        Path { p in
                            p.move(to: CGPoint(x: 14, y: 11))
                            p.addLine(to: CGPoint(x: 14, y: 29))
                            p.addLine(to: CGPoint(x: 28, y: 20))
                            p.closeSubpath()
                        }
                        .fill(Tokens.bg)
                        .frame(width: 40, height: 40)
                    }
                }
                .frame(width: 40, height: 40)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(isPreviewPlaying
                                ? T("settings.voice.preview_stop_a11y")
                                : T("settings.voice.preview_play_a11y"))
            VStack(alignment: .leading, spacing: 3) {
                Text(T("settings.voice.preview-kicker"))
                    .font(.mono(10))
                    .tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                // Same sentence we ship to the synthesizer — quoted so
                // the user reads what they'll hear. Tracks the voice's
                // language (draft.language), not the interface, so the
                // user can audition a French voice with a French phrase
                // while keeping the UI in Italian.
                Text("\u{201C}\(Self.previewSentence(forLang: draft.language))\u{201D}")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.text)
                    .lineSpacing(3)
            }
            Spacer()
            // Real waveform sampled from the just-synthesized WAV.
            // Wrapped in a TimelineView that polls the AVAudioPlayer's
            // `currentTime` at ~20Hz and converts it to `playedFraction`
            // — bars before that fraction render in accent, after it
            // in dim → the bar visibly "fills" left-to-right while the
            // audio plays. Empty `liveLevels` (before any preview ran)
            // collapses to a flat strip per the shared widget.
            TimelineView(.periodic(from: Date(), by: 0.05)) { _ in
                let fraction: Double = {
                    guard isPreviewPlaying,
                          let p = previewPlayer,
                          p.duration > 0
                    else { return 0 }
                    return min(1.0, p.currentTime / p.duration)
                }()
                Waveform(
                    count: 36,
                    playedFraction: fraction,
                    accent: accent.main,
                    dim: accent.main.opacity(0.35),
                    seed: 0.6, minHeight: 3, maxBump: 18,
                    liveLevels: previewWaveformLevels
                )
                .frame(width: 160, height: 28)
            }
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

    /// Tap handler for the preview play/stop button. If audio is
    /// currently playing → stop and clear state. Otherwise kick off
    /// `previewVoice()` (synth + play).
    private func togglePreview() {
        if isPreviewPlaying {
            previewPlayer?.stop()
            previewPlayer = nil
            isPreviewPlaying = false
            host.aecClearRenderReference()
            host.pushMessage("Anteprima voce: stop")
            return
        }
        if !previewing {
            previewVoice()
        }
    }

    private func previewVoice() {
        previewing = true
        // Sentence tracks the voice's language (draft.language) so
        // changing "Lingua" above swaps both the on-screen caption and
        // the synthesized audio — letting the user audition a voice in
        // its own locale while keeping the UI in another. Falls back to
        // English when the locale isn't covered by the table.
        let sample = Self.previewSentence(forLang: draft.language)
        host.pushMessage("Anteprima voce: \(draft.voice)…")
        Task {
            defer { Task { @MainActor in previewing = false } }
            do {
                let path = try await host.synthesizePreview(text: sample, voice: draft.voice)
                if path.isEmpty {
                    await MainActor.run {
                        host.pushMessage("Anteprima voce: path vuoto")
                        errorMessage = "Sintesi non riuscita."
                    }
                    return
                }
                guard FileManager.default.fileExists(atPath: path) else {
                    await MainActor.run {
                        host.pushMessage("Anteprima voce: file non trovato (\(path))")
                        errorMessage = "File audio non trovato."
                    }
                    return
                }
                // Pre-compute the real per-bucket waveform off the main
                // thread — reading the WAV is a few KB, but decoding +
                // bucketing should still not block UI updates.
                let levels = Self.waveformLevels(fromAudioFile: path, buckets: 36)
                await MainActor.run {
                    previewWaveformLevels = levels
                    let url = URL(fileURLWithPath: path)
                    previewPlayer?.stop()
                    previewPlayer = nil
                    let hint: String? = (url.pathExtension.lowercased() == "flac")
                        ? "org.xiph.flac" : nil
                    guard let player = try? AVAudioPlayer(contentsOf: url, fileTypeHint: hint),
                          player.prepareToPlay(),
                          player.play()
                    else {
                        host.pushMessage("Anteprima voce: AVAudioPlayer non ha avviato il file")
                        errorMessage = "Riproduzione non avviata."
                        return
                    }
                    // Feed the AEC render reference so SFSpeechRecognizer
                    // doesn't pick up the preview as a voice command —
                    // mirrors the note playback path. No-op when AEC
                    // isn't running (non-Apple STT).
                    host.aecSetRenderReference(path: path)
                    previewPlayer = player
                    isPreviewPlaying = true
                    let duration = player.duration
                    host.pushMessage(String(
                        format: "Anteprima voce: riproduco %@ (%.1fs)",
                        (path as NSString).lastPathComponent, duration
                    ))
                    // Auto-flip the toggle back to "play" when the
                    // audio finishes naturally. User stop short-
                    // circuits via togglePreview() which clears
                    // `isPreviewPlaying` first → this Task no-ops.
                    Task {
                        try? await Task.sleep(for: .seconds(max(duration, 0.5) + 0.3))
                        await MainActor.run {
                            if previewPlayer != nil {
                                previewPlayer = nil
                                isPreviewPlaying = false
                                host.aecClearRenderReference()
                            }
                        }
                    }
                }
            } catch {
                await MainActor.run {
                    host.pushMessage("Anteprima voce: errore \(error.localizedDescription)")
                    errorMessage = "Anteprima fallita: \(error.localizedDescription)"
                }
            }
        }
    }

    /// Compact a `$HOME`-prefixed absolute path into a `~/…` form for
    /// display. Pure cosmetic — `Reveal-in-Finder` and the help-tooltip
    /// still operate on the original absolute path. No-op when the path
    /// doesn't start with the home directory.
    private static func tildeify(_ path: String) -> String {
        let home = NSHomeDirectory()
        if path.hasPrefix(home) {
            return "~" + path.dropFirst(home.count)
        }
        return path
    }

    /// Preview phrase for a voice locale (BCP-47, e.g. "it-IT", "en-GB").
    /// Matches the language the *voice* speaks, not the UI — so the user
    /// can audition each voice in its own locale. Region-specific entries
    /// (`pt-BR`/`pt-PT`, `zh-CN`/`zh-TW`) come first; if the full tag
    /// misses we fall back to the primary subtag, then to English.
    private static func previewSentence(forLang bcp47: String) -> String {
        let full = bcp47.lowercased()
        switch full {
        case "pt-br": return "O tempo, no alto da serra, não é o tempo da planície."
        case "pt-pt": return "O tempo, na alta montanha, não é o tempo da planície."
        case "zh-cn": return "高山上的时间，不是平原上的时间。"
        case "zh-tw": return "高山上的時間，不是平原上的時間。"
        default: break
        }
        let primary = String(full.prefix(2))
        switch primary {
        case "it": return "Il tempo, nell'alta montagna, non è il tempo della pianura."
        case "en": return "Time, high in the mountains, is not the time of the lowlands."
        case "fr": return "Le temps, en haute montagne, n'est pas le temps de la plaine."
        case "de": return "Die Zeit, hoch in den Bergen, ist nicht die Zeit der Täler."
        case "es": return "El tiempo, en la alta montaña, no es el tiempo de la llanura."
        case "pt": return "O tempo, na alta montanha, não é o tempo da planície."
        case "ja": return "高山の時は、平地の時とは異なる。"
        case "zh": return "高山上的时间，不是平原上的时间。"
        case "hi": return "ऊँचे पहाड़ों का समय मैदानों का समय नहीं है।"
        default:   return "Time, high in the mountains, is not the time of the lowlands."
        }
    }

    /// Read a WAV/FLAC at `path` and downsample its absolute amplitudes
    /// into `buckets` peak values (0…1). Used to draw a real waveform
    /// for the Settings voice preview.
    private static func waveformLevels(fromAudioFile path: String, buckets: Int) -> [Float] {
        let url = URL(fileURLWithPath: path)
        guard let file = try? AVAudioFile(forReading: url) else { return [] }
        let format = file.processingFormat
        let frameCount = AVAudioFrameCount(file.length)
        guard frameCount > 0,
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount)
        else { return [] }
        do { try file.read(into: buffer) } catch { return [] }
        guard let channels = buffer.floatChannelData else { return [] }
        let count = Int(buffer.frameLength)
        guard count > 0 else { return [] }
        let samples = UnsafeBufferPointer(start: channels[0], count: count)
        let bucketSize = max(1, count / buckets)
        var levels: [Float] = []
        levels.reserveCapacity(buckets)
        for i in 0..<buckets {
            let start = i * bucketSize
            let end = min(count, start + bucketSize)
            var peak: Float = 0
            for j in start..<end {
                let v = abs(samples[j])
                if v > peak { peak = v }
            }
            levels.append(min(peak, 1.0))
        }
        return levels
    }

    private static func playAudio(at path: String) {
        #if canImport(AppKit)
        PreviewSoundCache.play(path: path)
        #endif
    }

    /// Reset the in-place audio player when the user navigates away
    /// from Settings, so a long preview doesn't keep playing in the
    /// background. Currently only PreviewSoundCache is used; future
    /// hooks could explicitly stop the active player here.
    private func stopPreview() {
        #if canImport(AppKit)
        PreviewSoundCache.stopAll()
        #endif
        previewPlayer?.stop()
        previewPlayer = nil
        isPreviewPlaying = false
        host.aecClearRenderReference()
    }

    private var sttSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionHeader(
                kicker: "003",
                title: T("settings.nav.stt"),
                sub: T("settings.sub.stt"),
                info: T("settings.info.stt")
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
                        Text(T("settings.stt.apple-requires"))
                        Text(T("settings.stt.open-sys-prefs"))
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
                kicker: "004",
                title: T("settings.nav.commands"),
                sub: T("settings.sub.commands"),
                info: T("settings.info.commands")
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
        // Save-on-edit: every chip add/remove updates `draftCommands`,
        // and we mirror the change straight into `host.voiceCommands`
        // (which the Swift voice-command resolver reads on the hot
        // path) + persist to TOML. The Apple helper itself stays dumb
        // (transcribes everything as `CMD <text>`); the host-side
        // resolver against the freshly-updated triggers is what makes
        // the new alias work the next time the user speaks it. No
        // engine restart needed for the alias to take effect.
        .onChange(of: draftCommands) { _, newValue in
            if newValue != host.voiceCommands {
                host.saveVoiceCommands(newValue)
            }
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
                kicker: "005",
                title: T("settings.sec.text-prep.title"),
                sub: T("settings.sub.text-prep"),
                info: T("settings.info.text-prep")
            )
            volumeSlider
            chunkSizeSlider
            PathRow(label: T("settings.text-prep.library"),
                    value: Self.tildeify(host.libraryDatabasePath),
                    tag: String(format: T("settings.text-prep.tag.sqlite"),
                                fileSize(host.libraryDatabasePath)),
                    action: nil, accent: accent,
                    onBrowse: nil,
                    onRevealInFinder: { revealInFinder(host.libraryDatabasePath) })
            PathRow(label: T("settings.text-prep.cache-audio"),
                    value: Self.tildeify(host.ttsCacheDir),
                    tag: String(format: T("settings.text-prep.tag.cache-audio"),
                                cacheDirSize()),
                    action: T("settings.text-prep.action.clear"), accent: accent,
                    onBrowse: nil,
                    onAction: { showClearCacheConfirm = true },
                    onRevealInFinder: { revealInFinder(host.ttsCacheDir) })
            PathRow(label: T("settings.text-prep.cache-notes"),
                    value: Self.tildeify(host.notesAudioDir),
                    tag: String(format: T("settings.text-prep.tag.cache-notes"),
                                directorySize(host.notesAudioDir)),
                    action: T("settings.text-prep.action.clear"), accent: accent,
                    onBrowse: nil,
                    onAction: {
                        notesConfirmAcknowledged = false
                        showClearNotesConfirm = true
                    },
                    onRevealInFinder: { revealInFinder(host.notesAudioDir) })
        }
        .sheet(isPresented: $showClearCacheConfirm) {
            clearCacheConfirmSheet
        }
        .sheet(isPresented: $showClearNotesConfirm) {
            clearNotesConfirmSheet
        }
    }

    /// Themed modal for the "svuota cache audio" action. Mirrors the
    /// look of `UrlImportSheet`: kicker + serif title + italic blurb +
    /// Annulla/Conferma buttons. Confirm calls `host.clearTtsCache()`
    /// off the main actor, then surfaces a transient toast with the
    /// number of bytes freed so the user has explicit feedback that
    /// something happened.
    private var clearCacheConfirmSheet: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                Text(T("settings.clear-cache.kicker"))
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text(T("settings.clear-cache.title"))
                    .font(.serif(22, weight: .medium))
                    .foregroundStyle(Tokens.text)
                Text(String(format: T("settings.clear-cache.body"), cacheDirSize()))
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack {
                Spacer()
                Button(T("settings.dialog.cancel")) { showClearCacheConfirm = false }
                    .keyboardShortcut(.cancelAction)
                    .disabled(clearingCache)
                Button(action: confirmClearCache) {
                    HStack(spacing: 8) {
                        if clearingCache {
                            ProgressView()
                                .progressViewStyle(.circular)
                                .controlSize(.small)
                                .tint(.red)
                        }
                        Text(clearingCache
                             ? T("settings.clear-cache.confirming")
                             : T("settings.clear-cache.confirm"))
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(clearingCache)
            }
        }
        .padding(28)
        .frame(width: 480)
        .background(Tokens.bg)
    }

    private func confirmClearCache() {
        guard !clearingCache else { return }
        clearingCache = true
        Task {
            let freed: Int64
            do {
                freed = try await host.clearTtsCache()
            } catch {
                await MainActor.run {
                    clearingCache = false
                    showClearCacheConfirm = false
                    host.transientToast = ToastMessage(
                        text: String(format: T("settings.toast.cache-failed"),
                                     error.localizedDescription),
                        kind: .error
                    )
                }
                return
            }
            await MainActor.run {
                clearingCache = false
                showClearCacheConfirm = false
                let bytes = ByteCountFormatter.string(
                    fromByteCount: freed, countStyle: .file
                )
                host.transientToast = ToastMessage(
                    text: freed > 0
                        ? String(format: T("settings.toast.cache-cleared"), bytes)
                        : T("settings.toast.cache-empty"),
                    kind: .info
                )
            }
        }
    }

    /// Themed modal for the **destructive** "svuota cache note" action.
    /// Distinguishing details vs the cache-audio sheet:
    ///   • red kicker / glyph instead of neutral
    ///   • two-step confirmation: an explicit checkbox the user must
    ///     tick before "Svuota" enables, so a stray Enter/click can't
    ///     wipe their notes
    ///   • the blurb spells out exactly what disappears (audio + DB
    ///     rows, every document) and that the change is irreversible
    private var clearNotesConfirmSheet: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 11, weight: .bold))
                        .foregroundStyle(Color.red.opacity(0.85))
                    Text(T("settings.clear-notes.kicker"))
                        .font(.mono(10)).tracking(2)
                        .foregroundStyle(Color.red.opacity(0.85))
                }
                Text(T("settings.clear-notes.title"))
                    .font(.serif(22, weight: .medium))
                    .foregroundStyle(Tokens.text)
                // The body uses inline Markdown (** **, ` `) — Text(_:)
                // applied to a String literal renders it verbatim. We
                // build it from the localized template via
                // LocalizedStringKey to keep the bold + monospace path
                // styling.
                Text(LocalizedStringKey(
                    String(format: T("settings.clear-notes.body"),
                           Self.tildeify(host.notesAudioDir))
                ))
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)
            }

            // Two-step gate. Conferma stays disabled until ticked, even
            // if the user mashes Enter — the keyboard shortcut on the
            // button respects `.disabled`.
            AccentCheckbox(
                checked: $notesConfirmAcknowledged,
                label: T("settings.clear-notes.acknowledge"),
                accent: accent
            )

            HStack {
                Spacer()
                Button(T("settings.dialog.cancel")) { showClearNotesConfirm = false }
                    .keyboardShortcut(.cancelAction)
                    .disabled(clearingNotes)
                Button(action: confirmClearNotes) {
                    HStack(spacing: 8) {
                        if clearingNotes {
                            ProgressView()
                                .progressViewStyle(.circular)
                                .controlSize(.small)
                                .tint(.red)
                        }
                        Text(clearingNotes
                             ? T("settings.clear-notes.confirming")
                             : T("settings.clear-notes.confirm"))
                            .foregroundStyle(Color.red.opacity(0.9))
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(clearingNotes || !notesConfirmAcknowledged)
            }
        }
        .padding(28)
        .frame(width: 520)
        .background(Tokens.bg)
    }

    private func confirmClearNotes() {
        guard !clearingNotes, notesConfirmAcknowledged else { return }
        clearingNotes = true
        Task {
            let result: (deletedCount: Int, bytesFreed: Int64)
            do {
                result = try await host.clearAllNotes()
            } catch {
                await MainActor.run {
                    clearingNotes = false
                    showClearNotesConfirm = false
                    host.transientToast = ToastMessage(
                        text: String(format: T("settings.toast.notes-failed"),
                                     error.localizedDescription),
                        kind: .error
                    )
                }
                return
            }
            await MainActor.run {
                clearingNotes = false
                showClearNotesConfirm = false
                notesConfirmAcknowledged = false
                let bytes = ByteCountFormatter.string(
                    fromByteCount: result.bytesFreed, countStyle: .file
                )
                let count = result.deletedCount
                let text: String
                if count == 0 {
                    text = T("settings.toast.notes-empty")
                } else if count == 1 {
                    text = String(format: T("settings.toast.notes-cleared_one"), bytes)
                } else {
                    text = String(format: T("settings.toast.notes-cleared_other"),
                                  Int64(count), bytes)
                }
                host.transientToast = ToastMessage(text: text, kind: .info)
            }
        }
    }

    /// Open the given absolute path in Finder. If the path is a
    /// directory we pop it open; if it's a file we select it. Falls
    /// back to a no-op on non-AppKit builds (currently mac-only).
    private func revealInFinder(_ path: String) {
        #if canImport(AppKit)
        guard !path.isEmpty else { return }
        let url = URL(fileURLWithPath: path)
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: path, isDirectory: &isDir) {
            if isDir.boolValue {
                NSWorkspace.shared.open(url)
            } else {
                NSWorkspace.shared.activateFileViewerSelecting([url])
            }
        } else {
            // Path doesn't exist yet (e.g. cache dir not created
            // because nothing has been synthesized) — try the parent.
            let parent = url.deletingLastPathComponent()
            if FileManager.default.fileExists(atPath: parent.path) {
                NSWorkspace.shared.open(parent)
            } else {
                host.pushMessage("Percorso non esistente: \(path)")
            }
        }
        #endif
    }

    /// Playback volume slider, 0.0–1.0 (linear). Two-way bound to
    /// `host.volume` so the change reaches rodio immediately. Keyboard
    /// shortcuts (⌘↑/⌘↓) also write here through the menu.
    private var volumeSlider: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(T("settings.audio.volume"))
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

    /// Formatted total size of every regular file under `path` (recursive).
    /// Walks the dir synchronously — cheap for the expected order of
    /// magnitude (a few thousand small files). Returns "—" for an empty
    /// path, "0 byte" if the dir doesn't exist yet.
    private func directorySize(_ path: String) -> String {
        guard !path.isEmpty else { return "—" }
        let url = URL(fileURLWithPath: path)
        guard let enumerator = FileManager.default.enumerator(
            at: url,
            includingPropertiesForKeys: [.fileSizeKey, .isRegularFileKey]
        ) else { return "0 byte" }
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

    /// Formatted size of a single file (the SQLite library). "—" when
    /// the path is empty or the file isn't on disk yet (first run).
    private func fileSize(_ path: String) -> String {
        guard !path.isEmpty else { return "—" }
        guard let attrs = try? FileManager.default.attributesOfItem(atPath: path),
              let size = attrs[.size] as? Int64
        else { return "—" }
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }

    private func cacheDirSize() -> String {
        directorySize(host.ttsCacheDir)
    }

    private var chunkSizeSlider: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Text(T("settings.text-prep.chunk-size"))
                    .font(.sans(11))
                    .tracking(0.5)
                    .foregroundStyle(Tokens.textDim)
                Text(String(format: T("settings.text-prep.chunk-size.suffix"),
                            Int64(draftChunkChars)))
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
                Text(T("settings.text-prep.chunk-size.left"))
                Spacer()
                Text(T("settings.text-prep.chunk-size.right"))
            }
            .font(.mono(9))
            .foregroundStyle(Tokens.textFaint)
            if draftChunkChars != host.chunkTargetChars {
                Text(T("settings.text-prep.chunk-size.warn"))
                    .font(.serif(12, italic: true))
                    .foregroundStyle(accent.main)
                    .padding(.top, 4)
            }
        }
    }

    private var installationsSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "006",
                title: T("settings.nav.installations"),
                sub: T("settings.sub.installations"),
                info: T("settings.info.installations")
            )

            // Manual remote-catalog refresh. The voice list is otherwise
            // populated from the bundled `voices.manifest.json` (or from
            // a previously fetched `voices.cache.json`). Hitting this
            // pulls the current catalog from huggingface.co — useful when
            // Kokoro publishes new voices upstream after the .app was
            // built.
            catalogRefreshControl

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

    private var catalogRefreshControl: some View {
        HStack(spacing: 10) {
            Button(action: {
                Task { await host.refreshRemoteVoiceCatalog() }
            }) {
                HStack(spacing: 6) {
                    if host.catalogRefreshInflight {
                        ProgressView()
                            .scaleEffect(0.6)
                            .frame(width: 12, height: 12)
                    }
                    Text(host.catalogRefreshInflight
                         ? "Aggiornamento…"
                         : "Aggiorna lista voci")
                        .font(.serif(13))
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .stroke(accent.main.opacity(0.5), lineWidth: 0.5)
                )
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(host.catalogRefreshInflight)

            if let status = host.catalogRefreshStatus {
                Text(status)
                    .font(.serif(12, italic: true))
                    .foregroundStyle(Tokens.textFaint)
            }
            Spacer()
        }
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

    /// Group label for the voice-catalog disclosure headers. Uses the
    /// FULL BCP-47 for pairs that would otherwise collapse into a
    /// duplicated heading (`en-US` and `en-GB` both → "Voci inglesi"),
    /// mirroring the fix applied to the onboarding voice picker.
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

    @ViewBuilder
    private func voiceLanguageSection(
        _ group: (language: String, display: String, assets: [InstallableAsset])
    ) -> some View {
        let missing = group.assets.filter { !$0.installed }
        let installed = group.assets.filter { $0.installed }
        DisclosureGroup {
            installRowStack(group.assets)
                .padding(.top, 6)
        } label: {
            HStack(spacing: 8) {
                Text(group.display)
                    .font(.serif(14, italic: true))
                    .foregroundStyle(Tokens.text)
                Text("(\(installed.count)/\(group.assets.count))")
                    .font(.mono(10))
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                if !missing.isEmpty {
                    Button(T("settings.install.install_all")) {
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
                // Symmetric counterpart of "installa tutte" — only
                // shown when at least one voice in the group is on
                // disk, so single-voice groups don't get a redundant
                // bulk button. Border + colour are dimmed (textFaint /
                // line) so it reads as a secondary destructive action,
                // not an attention-grabber.
                if !installed.isEmpty {
                    Button(T("settings.install.uninstall_all")) {
                        for a in installed { host.uninstallAsset(a.id) }
                    }
                    .buttonStyle(.plain)
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 5)
                        .fill(Color.white.opacity(0.04)))
                    .overlay(RoundedRectangle(cornerRadius: 5)
                        .strokeBorder(Tokens.line, lineWidth: 1))
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
                kicker: "007",
                title: T("settings.sec.theme.title"),
                sub: T("settings.sub.theme"),
                info: T("settings.info.theme")
            )
            // Two columns wrap nicely under the Settings content
            // width — three was too tight and the third card got
            // clipped on narrow windows.
            LazyVGrid(columns: [
                GridItem(.flexible(), spacing: 12),
                GridItem(.flexible(), spacing: 12)
            ], spacing: 12) {
                ForEach(AppTheme.all, id: \.id) { theme in
                    themeCard(theme)
                }
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
                Text(T("settings.theme.accent-tint"))
                    .font(.mono(10)).tracking(1.2)
                    .foregroundStyle(Tokens.textFaint)
                Spacer()
                Text("\(Int(customHue))°")
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .monospacedDigit()
            }
            Slider(value: $customHue, in: 0...360)
                .tint(Accent(hue: customHue).main)
                // Live update: when the user drags, every value step
                // pushes the new hue into the parent's `accent`
                // binding so the whole UI repaints in real time.
                // Previously we tried bumping `themeId` only on
                // editing-end, which (a) didn't fire during drag and
                // (b) reset the AppStorage round-trip even for
                // identity → no visible change.
                .onChange(of: customHue) { _, newHue in
                    if themeId == AppTheme.customId {
                        accent = Accent(hue: newHue)
                    }
                }
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
            .frame(maxWidth: .infinity, alignment: .leading)
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
                kicker: "008",
                title: T("settings.nav.interface"),
                sub: T("settings.sub.interface"),
                info: T("settings.info.interface")
            )

            HStack(spacing: 10) {
                ForEach(InterfaceLanguage.Code.allCases, id: \.rawValue) { code in
                    interfaceLangPill(code)
                }
                Spacer()
            }
            // No restart hint: `T(...)` reads the override at every call
            // and SwiftUI re-renders on the @AppStorage change, so the
            // whole UI flips language live.
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
    }

    private var diagnosticsSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            SectionHeader(
                kicker: "009",
                title: T("settings.nav.diagnostics"),
                sub: T("settings.sub.diagnostics"),
                info: T("settings.info.diagnostics")
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
                        Text(T("settings.diagnostics.export-log"))
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
                        Text(T("settings.diagnostics.restore-defaults"))
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
                Text(applying ? T("settings.top.applying") : T("settings.top.apply"))
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
    /// Computed (not `static let`) so the labels resolve through `T(...)`
    /// at every access — keeps the sidebar in sync with the in-session
    /// interface-language override without a relaunch. The lookup is
    /// cheap (a UserDefaults read + bundle string fetch).
    static var all: [SettingsNavEntry] {
        [
            .init(key: "language",      label: T("settings.nav.language")),
            .init(key: "voice",         label: T("settings.nav.voice")),
            .init(key: "stt",           label: T("settings.nav.stt")),
            .init(key: "commands",      label: T("settings.nav.commands")),
            .init(key: "audio",         label: T("settings.nav.audio")),
            .init(key: "theme",         label: T("settings.nav.theme")),
            .init(key: "installations", label: T("settings.nav.installations")),
            .init(key: "interface",     label: T("settings.nav.interface")),
            .init(key: "diagnostics",   label: T("settings.nav.diagnostics")),
        ]
    }
}
