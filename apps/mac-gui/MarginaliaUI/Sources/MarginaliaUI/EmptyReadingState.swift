import SwiftUI

/// Shown in the reading panel when no session is active. Two variants:
///
/// - **empty library**: first-run post-onboarding — hero + import CTA.
/// - **with library**: the user has documents but hasn't opened one yet —
///   gentle prompt pointing at the sidebar.
///
/// This replaces the `ReadingMock` fallback that used to leak through
/// `ReadingView` when `host.currentSession` was nil. Placing the empty
/// state at the `MarginaliaWindow` level keeps `ReadingView` strictly
/// about rendering a live session.
public struct EmptyReadingState: View {
    public var accent: Accent
    public var hasLibrary: Bool
    public var onImport: () -> Void

    public init(accent: Accent, hasLibrary: Bool, onImport: @escaping () -> Void) {
        self.accent = accent
        self.hasLibrary = hasLibrary
        self.onImport = onImport
    }

    public var body: some View {
        ZStack {
            // Subtle ambient — same radial wash ReadingView uses so the
            // mode-switch doesn't feel like walking into a blank room.
            RadialGradient(
                colors: [accent.main.opacity(0.06), .clear],
                center: .center, startRadius: 60, endRadius: 520
            )
            .blur(radius: 32)

            VStack(spacing: 26) {
                Spacer()
                VStack(spacing: 10) {
                    Text(hasLibrary ? "Scegli un documento" : "Inizia a leggere")
                        .font(.serif(36, italic: true))
                        .kerning(-0.4)
                        .foregroundStyle(Tokens.text)
                    Text(subtitle)
                        .font(.serif(15, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .lineSpacing(4)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 440)
                }

                if !hasLibrary {
                    Button(action: onImport) {
                        HStack(spacing: 8) {
                            Image(systemName: "plus")
                                .font(.system(size: 11, weight: .semibold))
                            Text("importa")
                                .font(.sans(13, weight: .medium))
                        }
                        .foregroundStyle(Tokens.bg)
                        .padding(.horizontal, 22).padding(.vertical, 10)
                        .background(Capsule().fill(accent.main))
                    }
                    .buttonStyle(.plain)
                    .keyboardShortcut("o", modifiers: [.command])

                    Text("oppure trascina un PDF, EPUB o .txt in questa finestra")
                        .font(.serif(12, italic: true))
                        .foregroundStyle(Tokens.textFaint)
                }
                Spacer()
            }
            .padding(.horizontal, 88)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Tokens.bg)
    }

    private var subtitle: String {
        if hasLibrary {
            return "Seleziona un titolo dalla libreria a sinistra per iniziare ad ascoltare. La lettura riprende dal punto in cui eri, se già avviata."
        } else {
            return "La libreria è vuota. Importa il primo documento per iniziare — Marginalia lo chunk, legge ad alta voce, e ricorda dove eri."
        }
    }
}
