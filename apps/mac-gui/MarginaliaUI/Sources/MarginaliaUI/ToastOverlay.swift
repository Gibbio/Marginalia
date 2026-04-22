import SwiftUI

/// Transient toast notification anchored to the top-right of the window.
/// Clears automatically ~3.2 s after appearance with a fade. Bound to the
/// host's `transientToast` @Published so any code path can trigger it by
/// setting the field — typically `pushMessage("Errore: …")` does it for you.
public struct ToastOverlay: View {
    @Binding public var toast: ToastMessage?
    public var accent: Accent

    @State private var visible: Bool = false

    public init(toast: Binding<ToastMessage?>, accent: Accent) {
        self._toast = toast
        self.accent = accent
    }

    public var body: some View {
        ZStack(alignment: .topTrailing) {
            Color.clear  // transparent layer so we can overlay without capturing hits
            if let t = toast {
                HStack(alignment: .top, spacing: 10) {
                    icon(for: t.kind)
                        .frame(width: 22, height: 22)
                    VStack(alignment: .leading, spacing: 6) {
                        Text(t.text)
                            .font(.serif(14))
                            .foregroundStyle(Tokens.text)
                            .lineLimit(4)
                            .frame(maxWidth: 360, alignment: .leading)
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
                .padding(.top, 48).padding(.trailing, 20)
                .transition(.move(edge: .trailing).combined(with: .opacity))
                .onAppear {
                    visible = true
                    Task {
                        try? await Task.sleep(for: .milliseconds(3200))
                        // Only dismiss if the toast we showed is still the
                        // same one (a newer toast should stay put).
                        if t.id == toast?.id { dismiss() }
                    }
                }
            }
        }
        .animation(.spring(duration: 0.35, bounce: 0.2), value: toast?.id)
        .allowsHitTesting(toast != nil)
    }

    private func dismiss() {
        toast = nil
        visible = false
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
