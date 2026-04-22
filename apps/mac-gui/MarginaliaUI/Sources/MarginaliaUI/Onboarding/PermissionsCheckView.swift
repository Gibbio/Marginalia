import SwiftUI
#if canImport(AppKit)
import AppKit
#endif
#if canImport(AVFoundation)
import AVFoundation
#endif
#if canImport(Speech)
import Speech
#endif

/// Pre-flight permissions step between Welcome and InstallModels.
///
/// Marginalia cannot function without mic access (AEC capture) + speech
/// recognition authorization (Apple STT). On-demand prompts later in the
/// flow are disruptive and give the user no recovery path if they decline
/// by reflex, so we gate onboarding here and provide a clear "Apri
/// Impostazioni di Sistema" fallback when something's denied.
///
/// Dictation is a soft check — Apple STT requires macOS Dictation to be on
/// in System Settings, but there's no public API to toggle it. We surface
/// `SFSpeechRecognizer.isAvailable` as a proxy (false usually means
/// Dictation is disabled or the user's locale isn't downloaded), with a
/// shortcut to the Keyboard preference pane. The user can proceed anyway
/// — the error will resurface at runtime if it matters.
public struct PermissionsCheckView: View {
    public var accent: Accent
    public var onContinue: () -> Void
    public var onBack: () -> Void

    public init(accent: Accent, onContinue: @escaping () -> Void, onBack: @escaping () -> Void) {
        self.accent = accent
        self.onContinue = onContinue
        self.onBack = onBack
    }

    public enum Status: Equatable {
        case unknown, granted, denied, restricted
    }

    @State private var micStatus: Status = .unknown
    @State private var speechStatus: Status = .unknown
    @State private var dictationAvailable: Bool? = nil    // nil until probed
    @State private var micProbing = false
    @State private var speechProbing = false

    public var body: some View {
        VStack(alignment: .leading, spacing: 28) {
            header

            VStack(spacing: 0) {
                permissionRow(
                    icon: "mic.fill",
                    title: "Microfono",
                    subtitle: micSubtitle,
                    status: micStatus,
                    probing: micProbing,
                    action: micAction,
                    actionLabel: micActionLabel
                )
                Divider().frame(height: 1).overlay(Tokens.lineSoft)
                permissionRow(
                    icon: "waveform",
                    title: "Riconoscimento vocale",
                    subtitle: speechSubtitle,
                    status: speechStatus,
                    probing: speechProbing,
                    action: speechAction,
                    actionLabel: speechActionLabel
                )
                Divider().frame(height: 1).overlay(Tokens.lineSoft)
                dictationRow
            }
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))

            rationale

            HStack(spacing: 12) {
                Button(action: onContinue) {
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

                Button(action: onBack) {
                    Text("indietro")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.horizontal, 10).padding(.vertical, 6)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 88).padding(.vertical, 60)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Tokens.bg)
        .onAppear { probeInitialStatus() }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("PERMESSI")
                .font(.mono(10)).tracking(2)
                .foregroundStyle(Tokens.textFaint)
            Text("Servono due autorizzazioni")
                .font(.serif(32, weight: .medium))
                .kerning(-0.3)
                .foregroundStyle(Tokens.text)
            Text("Marginalia ti ascolta per i comandi vocali e per trascrivere le note. Senza il microfono e il riconoscimento vocale l'app può ancora leggere, ma perde il cuore — interrompere con la voce, dire \"nota\" e parlare.")
                .font(.serif(15, italic: true))
                .foregroundStyle(Tokens.textDim)
                .lineSpacing(4)
                .frame(maxWidth: 620, alignment: .leading)
        }
    }

    private var rationale: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "lock.shield")
                .font(.system(size: 11))
                .foregroundStyle(Tokens.textFaint)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text("AUDIO LAVORATO IN LOCALE")
                    .font(.mono(10)).tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                Text("La voce non lascia mai il Mac: il microfono passa dall'echo-cancellation in Rust al riconoscimento vocale di macOS, entrambi on-device. Nessun audio viene caricato in rete.")
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

    private var canProceed: Bool {
        micStatus == .granted && speechStatus == .granted
    }

    // MARK: — Rows

    @ViewBuilder
    private func permissionRow(
        icon: String,
        title: String,
        subtitle: String,
        status: Status,
        probing: Bool,
        action: @escaping () -> Void,
        actionLabel: String?
    ) -> some View {
        HStack(spacing: 14) {
            statusDot(for: status, probing: probing, glyph: icon)
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.serif(15, italic: status == .granted))
                    .foregroundStyle(Tokens.text)
                Text(subtitle)
                    .font(.mono(10))
                    .foregroundStyle(subtitleColor(for: status))
            }
            Spacer()
            if status != .granted, let label = actionLabel {
                Button(action: action) {
                    Text(label)
                        .font(.mono(11))
                        .foregroundStyle(accent.main)
                        .padding(.horizontal, 10).padding(.vertical, 4)
                        .background(RoundedRectangle(cornerRadius: 5)
                            .fill(accent.main.opacity(0.08)))
                        .overlay(RoundedRectangle(cornerRadius: 5)
                            .strokeBorder(accent.main.opacity(0.3), lineWidth: 1))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
    }

    private var dictationRow: some View {
        HStack(spacing: 14) {
            dictationStatusDot
            VStack(alignment: .leading, spacing: 2) {
                Text("Dettatura di macOS")
                    .font(.serif(15, italic: dictationAvailable == true))
                    .foregroundStyle(Tokens.text)
                Text(dictationSubtitle)
                    .font(.mono(10))
                    .foregroundStyle(dictationAvailable == false
                        ? Color.orange.opacity(0.85) : Tokens.textFaint)
            }
            Spacer()
            if dictationAvailable == false {
                Button(action: openDictationSettings) {
                    Text("apri impostazioni")
                        .font(.mono(11))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.horizontal, 10).padding(.vertical, 4)
                        .background(RoundedRectangle(cornerRadius: 5)
                            .fill(Color.white.opacity(0.04)))
                        .overlay(RoundedRectangle(cornerRadius: 5)
                            .strokeBorder(Tokens.line, lineWidth: 1))
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
    }

    @ViewBuilder
    private func statusDot(for status: Status, probing: Bool, glyph: String) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(fillColor(for: status))
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(strokeColor(for: status), lineWidth: 1)
            if probing {
                ProgressView()
                    .progressViewStyle(.circular)
                    .controlSize(.small)
                    .tint(accent.main)
            } else {
                Image(systemName: statusGlyph(for: status, fallback: glyph))
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(statusGlyphColor(for: status))
            }
        }
        .frame(width: 26, height: 26)
    }

    private var dictationStatusDot: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(dictationAvailable == true ? accent.main.opacity(0.15) : Color.white.opacity(0.03))
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(
                    dictationAvailable == true ? accent.main.opacity(0.35)
                        : (dictationAvailable == false ? Color.orange.opacity(0.35) : Tokens.line),
                    lineWidth: 1
                )
            if dictationAvailable == nil {
                ProgressView().controlSize(.small).tint(accent.main)
            } else {
                Image(systemName: dictationAvailable == true ? "checkmark" : "exclamationmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(dictationAvailable == true ? accent.main : .orange.opacity(0.85))
            }
        }
        .frame(width: 26, height: 26)
    }

    private func fillColor(for s: Status) -> Color {
        switch s {
        case .granted:     return accent.main.opacity(0.15)
        case .denied, .restricted: return Color.red.opacity(0.08)
        case .unknown:     return Color.white.opacity(0.03)
        }
    }
    private func strokeColor(for s: Status) -> Color {
        switch s {
        case .granted:     return accent.main.opacity(0.35)
        case .denied, .restricted: return Color.red.opacity(0.4)
        case .unknown:     return Tokens.line
        }
    }
    private func statusGlyph(for s: Status, fallback: String) -> String {
        switch s {
        case .granted:     return "checkmark"
        case .denied, .restricted: return "exclamationmark"
        case .unknown:     return fallback
        }
    }
    private func statusGlyphColor(for s: Status) -> Color {
        switch s {
        case .granted:     return accent.main
        case .denied, .restricted: return .red.opacity(0.85)
        case .unknown:     return Tokens.textFaint
        }
    }
    private func subtitleColor(for s: Status) -> Color {
        switch s {
        case .denied, .restricted: return .red.opacity(0.85)
        default: return Tokens.textFaint
        }
    }

    // MARK: — Subtitles and actions

    private var micSubtitle: String {
        switch micStatus {
        case .granted:    return "autorizzato"
        case .denied:     return "negato · apri le impostazioni per concedere l'accesso"
        case .restricted: return "limitato dal sistema"
        case .unknown:    return "serve per i comandi vocali e le note dettate"
        }
    }
    private var micActionLabel: String? {
        switch micStatus {
        case .unknown: return "autorizza"
        case .denied, .restricted: return "apri impostazioni"
        case .granted: return nil
        }
    }
    private func micAction() {
        switch micStatus {
        case .unknown: requestMic()
        case .denied, .restricted: openPrivacyMicSettings()
        case .granted: break
        }
    }

    private var speechSubtitle: String {
        switch speechStatus {
        case .granted:    return "autorizzato"
        case .denied:     return "negato · apri le impostazioni per concedere l'accesso"
        case .restricted: return "limitato dal sistema"
        case .unknown:    return "trascrive la voce in comandi e in testo delle note"
        }
    }
    private var speechActionLabel: String? {
        switch speechStatus {
        case .unknown: return "autorizza"
        case .denied, .restricted: return "apri impostazioni"
        case .granted: return nil
        }
    }
    private func speechAction() {
        switch speechStatus {
        case .unknown: requestSpeech()
        case .denied, .restricted: openPrivacySpeechSettings()
        case .granted: break
        }
    }

    private var dictationSubtitle: String {
        switch dictationAvailable {
        case .some(true):  return "disponibile"
        case .some(false): return "non disponibile · attiva la Dettatura in Preferenze tastiera"
        case .none:        return "verifico…"
        }
    }

    // MARK: — Probes

    private func probeInitialStatus() {
        micStatus = currentMicStatus()
        speechStatus = currentSpeechStatus()
        probeDictation()
    }

    private func currentMicStatus() -> Status {
        #if canImport(AVFoundation)
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized:      return .granted
        case .denied:          return .denied
        case .restricted:      return .restricted
        case .notDetermined:   return .unknown
        @unknown default:      return .unknown
        }
        #else
        return .granted
        #endif
    }

    private func currentSpeechStatus() -> Status {
        #if canImport(Speech)
        switch SFSpeechRecognizer.authorizationStatus() {
        case .authorized:    return .granted
        case .denied:        return .denied
        case .restricted:    return .restricted
        case .notDetermined: return .unknown
        @unknown default:    return .unknown
        }
        #else
        return .granted
        #endif
    }

    private func requestMic() {
        #if canImport(AVFoundation)
        micProbing = true
        AVCaptureDevice.requestAccess(for: .audio) { granted in
            DispatchQueue.main.async {
                micProbing = false
                micStatus = granted ? .granted : .denied
            }
        }
        #endif
    }

    private func requestSpeech() {
        #if canImport(Speech)
        speechProbing = true
        SFSpeechRecognizer.requestAuthorization { authStatus in
            DispatchQueue.main.async {
                speechProbing = false
                switch authStatus {
                case .authorized:  speechStatus = .granted
                case .denied:      speechStatus = .denied
                case .restricted:  speechStatus = .restricted
                case .notDetermined: speechStatus = .unknown
                @unknown default:  speechStatus = .unknown
                }
                // Re-probe dictation availability — the SFSpeechRecognizer
                // availability flag can only be queried after authorization.
                probeDictation()
            }
        }
        #endif
    }

    private func probeDictation() {
        #if canImport(Speech)
        // Give SFSpeechRecognizer a beat to update after auth changes.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
            guard let recognizer = SFSpeechRecognizer(locale: Locale.current)
                ?? SFSpeechRecognizer()
            else {
                dictationAvailable = false
                return
            }
            dictationAvailable = recognizer.isAvailable
        }
        #else
        dictationAvailable = true
        #endif
    }

    // MARK: — External settings links

    private func openPrivacyMicSettings() {
        openURL("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
    }
    private func openPrivacySpeechSettings() {
        openURL("x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition")
    }
    private func openDictationSettings() {
        openURL("x-apple.systempreferences:com.apple.preference.keyboard?Dictation")
    }
    private func openURL(_ string: String) {
        #if canImport(AppKit)
        guard let url = URL(string: string) else { return }
        NSWorkspace.shared.open(url)
        #endif
    }
}
