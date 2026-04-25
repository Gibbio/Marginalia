import SwiftUI

/// Transient toast notification anchored to the top-right of the window.
/// Clears automatically ~3.2 s after appearance with a fade. Bound to the
/// host's `transientToast` @Published so any code path can trigger it by
/// setting the field — typically `pushMessage("Errore: …")` does it for you.
public struct ToastOverlay: View {
    @Binding public var toast: ToastMessage?
    public var accent: Accent

    public init(toast: Binding<ToastMessage?>, accent: Accent) {
        self._toast = toast
        self.accent = accent
    }

    public var body: some View {
        // Anchored bottom-right so transient action feedback (Pausa /
        // Riprendi / Avanti / …) lands above the margin-panel hint
        // ("passa sopra una nota per vedere il chunk collegato") rather
        // than at the top corner where it competed visually with the
        // toolbar. Errors and longer info toasts use the same anchor —
        // simpler than splitting kinds across screen edges.
        ZStack(alignment: .bottomTrailing) {
            // Layout anchor only — without a full-bleed view here the
            // ZStack would shrink to the toast card's frame and re-align.
            // `.allowsHitTesting(false)` is load-bearing: plain `Color.clear`
            // still participates in hit testing and would swallow every
            // click in the window while the toast is visible, blocking
            // e.g. the play button right after a pause toast appeared.
            Color.clear.allowsHitTesting(false)
            if let t = toast {
                HStack(alignment: .top, spacing: 10) {
                    icon(for: t.kind)
                        .frame(width: 22, height: 22)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(t.text)
                            .font(.serif(16))
                            .foregroundStyle(Tokens.text)
                            .lineLimit(3)
                            .frame(maxWidth: 320, alignment: .leading)
                        if let action = t.action {
                            Button(action: {
                                action.handler()
                                dismiss()
                            }) {
                                Text(action.label)
                                    .font(.mono(11))
                                    .foregroundStyle(strokeColor(for: t.kind))
                                    .padding(.horizontal, 10).padding(.vertical, 4)
                                    .background(RoundedRectangle(cornerRadius: 5)
                                        .fill(strokeColor(for: t.kind).opacity(0.1)))
                                    .overlay(RoundedRectangle(cornerRadius: 5)
                                        .strokeBorder(strokeColor(for: t.kind).opacity(0.4),
                                                      lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    Button(action: dismiss) {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .semibold))
                            .foregroundStyle(Tokens.textFaint)
                            .padding(4)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 14).padding(.vertical, 12)
                .background(
                    RoundedRectangle(cornerRadius: 10)
                        .fill(Tokens.bg2)
                        .shadow(color: .black.opacity(0.4), radius: 18, y: 6)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 10)
                        .strokeBorder(strokeColor(for: t.kind), lineWidth: 1)
                )
                // Sits just above the margin-panel footer hint row + the
                // collapsed LogPane bar. Trailing 20pt aligns with the
                // gearshape inset; bottom 72pt clears the hint and log
                // strip without overlapping them.
                .padding(.trailing, 20).padding(.bottom, 72)
                .transition(.move(edge: .bottom).combined(with: .opacity))
                .contentShape(Rectangle())
                .onTapGesture { dismiss() }
            }
        }
        .animation(.spring(duration: 0.35, bounce: 0.2), value: toast?.id)
        // Restart the auto-dismiss timer each time the toast id changes,
        // including when a new toast replaces an older one in-place (the
        // view is reused, so `.onAppear` wouldn't fire again). Previous
        // code attached the timer to `.onAppear` and a replacement toast
        // stayed forever because the first timer's dismiss guard saw a
        // mismatched id and skipped.
        .onChange(of: toast?.id) { _, newId in
            guard let id = newId else { return }
            Task {
                try? await Task.sleep(for: .milliseconds(3000))
                if id == toast?.id { dismiss() }
            }
        }
        .onAppear {
            if let id = toast?.id {
                Task {
                    try? await Task.sleep(for: .milliseconds(3000))
                    if id == toast?.id { dismiss() }
                }
            }
        }
    }

    private func dismiss() {
        toast = nil
    }

    @ViewBuilder
    private func icon(for kind: ToastMessage.Kind) -> some View {
        ZStack {
            Circle().fill(accent.main.opacity(0.15))
            Image(systemName: iconName(for: kind))
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(strokeColor(for: kind))
        }
    }

    private func iconName(for kind: ToastMessage.Kind) -> String {
        switch kind {
        case .info:    return "info"
        case .warning: return "exclamationmark"
        case .error:   return "xmark"
        }
    }

    private func strokeColor(for kind: ToastMessage.Kind) -> Color {
        switch kind {
        case .info:    return accent.main
        case .warning: return Color(red: 0.95, green: 0.75, blue: 0.4)
        case .error:   return Color(red: 0.92, green: 0.52, blue: 0.52)
        }
    }
}
