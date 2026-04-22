import SwiftUI

/// First-run welcome. Shown when `marginalia.toml` is missing. Transitions
/// to InstallModelsView when the user taps "inizia".
public struct WelcomeView: View {
    public var accent: Accent
    public var onContinue: () -> Void

    public init(accent: Accent, onContinue: @escaping () -> Void) {
        self.accent = accent; self.onContinue = onContinue
    }

    public var body: some View {
        ZStack {
            RadialGradient(
                colors: [accent.main.opacity(0.12), .clear],
                center: .center, startRadius: 50, endRadius: 700
            )
            .blur(radius: 28)
            .ignoresSafeArea()

            VStack(alignment: .leading, spacing: 18) {
                Text("BENVENUTO")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)

                VStack(alignment: .leading, spacing: 0) {
                    Text("Marginalia")
                        .font(.serif(72, italic: true))
                        .kerning(-1)
                        .foregroundStyle(Tokens.text)
                    Text("un lettore vocale che ascolta")
                        .font(.serif(28, italic: true))
                        .foregroundStyle(Tokens.textDim)
                }

                Text("Leggi un testo ad alta voce, interrompi con la voce, aggiungi note parlate che si ancorano al passaggio esatto che stai ascoltando. Tutto in locale, senza connessione.")
                    .font(.serif(17))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(5)
                    .frame(maxWidth: 560, alignment: .leading)
                    .padding(.top, 12)

                HStack(spacing: 14) {
                    Button(action: onContinue) {
                        Text("inizia")
                            .font(.sans(14, weight: .medium))
                            .foregroundStyle(Tokens.bg)
                            .padding(.horizontal, 24).padding(.vertical, 10)
                            .background(
                                Capsule().fill(accent.main)
                                    .shadow(color: accent.glow, radius: 14)
                            )
                    }
                    .buttonStyle(.plain)

                    Text("il prossimo passo scarica ~75 MB di modello vocale")
                        .font(.serif(13, italic: true))
                        .foregroundStyle(Tokens.textFaint)
                }
                .padding(.top, 28)
            }
            .padding(.horizontal, 88)
            .padding(.vertical, 60)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        }
        .background(Tokens.bg)
    }
}
