import SwiftUI

/// Footer log pane shown at the bottom of the window. Compact by default —
/// just the most recent message in dim mono — click to expand into a
/// scrollable list of the last 64 lines. Mirrors the TUI's sidebar log
/// (see `apps/tui-rs/src/main.rs:350-366`).
public struct LogPane: View {
    public var messages: [String]
    public var accent: Accent
    @Binding public var expanded: Bool

    public init(messages: [String], accent: Accent, expanded: Binding<Bool>) {
        self.messages = messages
        self.accent = accent
        self._expanded = expanded
    }

    public var body: some View {
        VStack(spacing: 0) {
            if expanded { expandedPane }
            footerBar
        }
        .background(Tokens.bg2)
        .overlay(alignment: .top) {
            Rectangle().fill(Tokens.line).frame(height: 1)
        }
    }

    private var footerBar: some View {
        HStack(spacing: 10) {
            Circle()
                .fill(accent.main)
                .frame(width: 5, height: 5)
                .shadow(color: accent.main, radius: 3)
            Text(messages.last ?? T("logpane.idle"))
                .font(.mono(11))
                .foregroundStyle(Tokens.textDim)
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button(action: { expanded.toggle() }) {
                HStack(spacing: 4) {
                    Text(T("logpane.toggle"))
                    Image(systemName: expanded ? "chevron.down" : "chevron.up")
                        .font(.system(size: 9, weight: .semibold))
                }
                .font(.mono(10))
                .foregroundStyle(Tokens.textFaint)
                .padding(.horizontal, 8).padding(.vertical, 2)
                .background(
                    RoundedRectangle(cornerRadius: 4)
                        .fill(Color.white.opacity(0.04))
                )
            }
            .buttonStyle(.plain)
            .help(expanded ? T("logpane.hide") : T("logpane.show"))
        }
        .padding(.horizontal, 14).padding(.vertical, 6)
    }

    private var expandedPane: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(messages.enumerated()), id: \.offset) { idx, msg in
                        Text(msg)
                            .font(.mono(11))
                            .foregroundStyle(messageColor(msg))
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .id(idx)
                    }
                }
                .padding(.horizontal, 14).padding(.vertical, 10)
            }
            .scrollIndicators(.automatic)
            .frame(height: 180)
            .background(Tokens.bg)
            .onChange(of: messages.count) { _, _ in
                if let last = messages.indices.last {
                    withAnimation(.easeOut(duration: 0.15)) {
                        proxy.scrollTo(last, anchor: .bottom)
                    }
                }
            }
        }
    }

    /// Visual differentiation for the common message prefixes used by the
    /// host's pushMessage sites (matches a bit of the TUI's colour coding).
    private func messageColor(_ msg: String) -> Color {
        if msg.hasPrefix("Errore") { return Color(red: 0.93, green: 0.55, blue: 0.55) }
        if msg.hasPrefix("→ ")     { return accent.main }
        if msg.hasPrefix("stt:")   { return Tokens.textDim }
        return Tokens.text.opacity(0.78)
    }
}
