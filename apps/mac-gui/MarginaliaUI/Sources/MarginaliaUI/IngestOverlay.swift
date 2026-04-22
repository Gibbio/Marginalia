import SwiftUI

/// Blocking overlay shown while the runtime is chunking a freshly
/// imported document. Drag-dropping a 50 MB PDF can take 10-20 s and
/// without this the user has no idea the app is working.
///
/// Design: semi-opaque veil over the whole window + centred card with
/// a circular spinner + document name. Matches the Inchiostro palette
/// (warm-dark bg, accent glow) used elsewhere.
public struct IngestOverlay: View {
    public var source: String
    public var accent: Accent

    public init(source: String, accent: Accent) {
        self.source = source
        self.accent = accent
    }

    public var body: some View {
        ZStack {
            // Veil — dims reading-column + sidebar but stays short of
            // full opacity so the user still sees where they are.
            Color.black.opacity(0.45)
                .ignoresSafeArea()

            VStack(spacing: 18) {
                ProgressView()
                    .progressViewStyle(.circular)
                    .controlSize(.large)
                    .tint(accent.main)
                VStack(spacing: 4) {
                    Text("STO LEGGENDO")
                        .font(.mono(10)).tracking(2)
                        .foregroundStyle(Tokens.textFaint)
                    Text(source)
                        .font(.serif(18, italic: true))
                        .foregroundStyle(Tokens.text)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: 420)
                    Text("i documenti lunghi richiedono qualche secondo")
                        .font(.serif(12, italic: true))
                        .foregroundStyle(Tokens.textDim)
                        .padding(.top, 2)
                }
            }
            .padding(.horizontal, 40).padding(.vertical, 32)
            .background(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(Tokens.bg)
                    .shadow(color: accent.glow.opacity(0.4), radius: 24)
            )
            .overlay(
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
        }
        .allowsHitTesting(true)  // intercept clicks — import is blocking
    }
}
