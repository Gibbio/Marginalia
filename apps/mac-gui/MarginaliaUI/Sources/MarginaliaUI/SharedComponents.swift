import SwiftUI

// MARK: — Waveform

/// Deterministic fake waveform: `count` bars, heights driven by a sine curve.
/// Mirrors the `Array.from({length: ...})` patterns in the JSX prototype.
public struct Waveform: View {
    public var count: Int = 30
    public var playedFraction: Double = 0.0   // 0 … 1 — amount shown in accent colour
    public var minHeight: CGFloat = 3
    public var maxBump:   CGFloat = 14        // added via |sin|
    public var accent: Color
    public var dim:    Color
    public var seed:   Double = 0.55          // phase multiplier
    /// Live amplitudes (each in 0…1). When non-nil, the bars are driven by
    /// these real values instead of the deterministic sine mock.
    public var liveLevels: [Float]? = nil

    public init(count: Int = 30, playedFraction: Double = 0.0,
                accent: Color, dim: Color, seed: Double = 0.55,
                minHeight: CGFloat = 3, maxBump: CGFloat = 14,
                liveLevels: [Float]? = nil) {
        self.count = count; self.playedFraction = playedFraction
        self.accent = accent; self.dim = dim; self.seed = seed
        self.minHeight = minHeight; self.maxBump = maxBump
        self.liveLevels = liveLevels
    }

    public var body: some View {
        HStack(alignment: .center, spacing: 1.5) {
            ForEach(0..<count, id: \.self) { i in
                let h = barHeight(i)
                let played = Double(i) < Double(count) * playedFraction
                let op = 0.3 + Double(i % 3) * 0.2
                Rectangle()
                    .fill(played ? accent : dim)
                    .frame(width: 1.5, height: h)
                    .opacity(played ? 0.9 : op)
            }
        }
        .frame(height: minHeight + maxBump + 4)
        .animation(.linear(duration: 0.1), value: liveLevels)
    }

    private func barHeight(_ i: Int) -> CGFloat {
        // Explicit empty array means "real source, currently silent" — render
        // flat minimum-height bars. The deterministic sine fallback kicks in
        // only when the caller passed no `liveLevels` at all (nil), i.e.
        // decorative contexts with no real source wired up.
        if let levels = liveLevels {
            guard !levels.isEmpty else { return minHeight }
            let idx = (i * levels.count) / max(count, 1)
            let v = CGFloat(max(0, min(1, levels[min(idx, levels.count - 1)])))
            return minHeight + v * maxBump * 1.6
        }
        let phase = Double(i) * seed
        return minHeight + CGFloat(abs(sin(phase))) * maxBump + CGFloat(i % 3) * 2
    }
}

// MARK: — Play button (accent-filled circle with triangle)

public struct PlayButton: View {
    public var accent: Color
    public var glow: Color
    public var size: CGFloat = 40

    public init(accent: Color, glow: Color, size: CGFloat = 40) {
        self.accent = accent; self.glow = glow; self.size = size
    }

    public var body: some View {
        ZStack {
            Circle()
                .fill(accent)
                .shadow(color: glow, radius: 10, x: 0, y: 0)
            // Triangle (right-pointing) matching the HTML prototype's CSS triangle.
            Path { p in
                let side: CGFloat = size * 0.32
                p.move(to: CGPoint(x: size/2 - side*0.2, y: size/2 - side))
                p.addLine(to: CGPoint(x: size/2 - side*0.2, y: size/2 + side))
                p.addLine(to: CGPoint(x: size/2 + side*0.6, y: size/2))
                p.closeSubpath()
            }
            .fill(Tokens.bg)
        }
        .frame(width: size, height: size)
    }
}

// MARK: — Gender icon (female: circle + skirt; male: circle + bars)

public struct GenderIcon: View {
    public var gender: Gender
    public var color: Color = Tokens.textDim

    public init(gender: Gender, color: Color = Tokens.textDim) {
        self.gender = gender; self.color = color
    }

    public var body: some View {
        Group {
            switch gender {
            case .female:
                Path { p in
                    // Head.
                    p.addEllipse(in: CGRect(x: 5, y: 2, width: 6, height: 6))
                    // Skirt outline.
                    p.move(to: CGPoint(x: 8, y: 8))
                    p.addLine(to: CGPoint(x: 8, y: 11))
                    p.addLine(to: CGPoint(x: 5, y: 18))
                    p.addLine(to: CGPoint(x: 11, y: 18))
                    p.addLine(to: CGPoint(x: 8, y: 11))
                }
                .stroke(color, lineWidth: 1.5)
            case .male, .unknown:
                Path { p in
                    p.addEllipse(in: CGRect(x: 5, y: 2, width: 6, height: 6))
                    p.move(to: CGPoint(x: 8, y: 8))
                    p.addLine(to: CGPoint(x: 8, y: 18))
                    p.move(to: CGPoint(x: 5, y: 13))
                    p.addLine(to: CGPoint(x: 11, y: 13))
                    p.move(to: CGPoint(x: 5, y: 18))
                    p.addLine(to: CGPoint(x: 11, y: 18))
                }
                .stroke(color, lineWidth: 1.5)
            }
        }
        .frame(width: 16, height: 22)
    }
}

// MARK: — Hint card (soft ink-blue callout)

public struct HintCard<Content: View>: View {
    public var accent: Accent
    @ViewBuilder public var content: () -> Content

    public init(accent: Accent, @ViewBuilder content: @escaping () -> Content) {
        self.accent = accent; self.content = content
    }

    public var body: some View {
        HStack(alignment: .top) { content() }
            .font(.serif(13, italic: true))
            .foregroundStyle(Tokens.text)
            .padding(.vertical, 12)
            .padding(.horizontal, 14)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(accent.main.opacity(0.08))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .strokeBorder(accent.main.opacity(0.25), lineWidth: 1)
            )
    }
}

// MARK: — Mini stat card (labelled value)

public struct MiniStat: View {
    public var label: String
    public var value: String
    public init(label: String, value: String) {
        self.label = label; self.value = value
    }
    public var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label.uppercased())
                .font(.mono(10))
                .tracking(1.2)
                .foregroundStyle(Tokens.textFaint)
            Text(value)
                .font(.serif(20))
                .foregroundStyle(Tokens.text)
        }
        .padding(.horizontal, 14).padding(.vertical, 12)
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
}

// MARK: — Accent checkbox

public struct AccentCheckbox: View {
    @Binding public var checked: Bool
    public var label: String
    public var accent: Accent

    public init(checked: Binding<Bool>, label: String, accent: Accent) {
        self._checked = checked; self.label = label; self.accent = accent
    }

    public var body: some View {
        Button(action: { checked.toggle() }) {
            HStack(spacing: 10) {
                ZStack {
                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                        .fill(checked ? accent.main : Color.clear)
                        .frame(width: 18, height: 18)
                        .overlay(
                            RoundedRectangle(cornerRadius: 4, style: .continuous)
                                .strokeBorder(checked ? accent.main : Tokens.textGhost,
                                              lineWidth: 1)
                        )
                    if checked {
                        Path { p in
                            p.move(to: CGPoint(x: 4, y: 9.5))
                            p.addLine(to: CGPoint(x: 7.5, y: 13))
                            p.addLine(to: CGPoint(x: 14, y: 6))
                        }
                        .stroke(Tokens.bg, style: StrokeStyle(lineWidth: 2,
                                lineCap: .round, lineJoin: .round))
                        .frame(width: 18, height: 18)
                    }
                }
                Text(label)
                    .font(.serif(15))
                    .foregroundStyle(Tokens.textDim)
            }
        }
        .buttonStyle(.plain)
    }
}

// MARK: — Chip (removable) used by the voice-commands editor

public struct Chip: View {
    public var text: String
    public var onRemove: () -> Void
    public init(text: String, onRemove: @escaping () -> Void) {
        self.text = text; self.onRemove = onRemove
    }
    public var body: some View {
        HStack(spacing: 6) {
            Text("\u{201C}\(text)\u{201D}")
                .font(.serif(14, italic: true))
                .foregroundStyle(Tokens.text)
            Button(action: onRemove) {
                Text("×")
                    .font(.system(size: 12))
                    .foregroundStyle(Tokens.textFaint)
                    .frame(width: 14, height: 14)
            }
            .buttonStyle(.plain)
        }
        .padding(.leading, 10)
        .padding(.trailing, 5)
        .padding(.vertical, 5)
        .background(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .fill(Color(hex: 0xEFE5CF, opacity: 0.05))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 6, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
    }
}

// MARK: — Small icon button

public struct IconBtn: View {
    public var label: String
    public var danger: Bool = false
    public var action: () -> Void
    public init(label: String, danger: Bool = false, action: @escaping () -> Void = {}) {
        self.label = label; self.danger = danger; self.action = action
    }
    public var body: some View {
        Button(action: action) {
            Text(label)
                .font(.sans(12))
                .foregroundStyle(danger ? Color(red: 0.93, green: 0.53, blue: 0.53) : Tokens.textDim)
                .padding(.horizontal, 10).padding(.vertical, 5)
                .background(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .fill(Color.clear)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: 6, style: .continuous)
                        .strokeBorder(
                            danger
                                ? Color(red: 0.91, green: 0.53, blue: 0.53).opacity(0.3)
                                : Tokens.line,
                            lineWidth: 1)
                )
        }
        .buttonStyle(.plain)
    }
}

// MARK: — Section kicker (numbered + title + subtitle) used by Settings

public struct SectionHeader: View {
    public var kicker: String
    public var title: String
    public var sub: String
    /// Longer-form help — rendered as a click-to-open popover anchored to
    /// an "info" chip next to the title. Click-outside dismisses it.
    public var info: String?

    @State private var showingInfo: Bool = false
    @State private var hovering: Bool = false

    public init(kicker: String, title: String, sub: String, info: String? = nil) {
        self.kicker = kicker; self.title = title; self.sub = sub; self.info = info
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(kicker)
                .font(.mono(10))
                .tracking(2)
                .foregroundStyle(Tokens.textFaint)
                .padding(.bottom, 2)
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                Text(title)
                    .font(.serif(26, weight: .medium))
                    .kerning(-0.3)
                    .foregroundStyle(Tokens.text)
                if let info {
                    infoChip(info)
                }
            }
            Text(sub)
                .font(.serif(16, italic: true))
                .foregroundStyle(Tokens.textDim)
                .lineSpacing(3)
                .frame(maxWidth: 560, alignment: .leading)
        }
        .padding(.bottom, 22)
    }

    /// A small pill — icon + "cos'è?" — sized for comfortable clicking,
    /// with hover state and a custom-styled popover on tap. Replaces the
    /// old native tooltip behaviour.
    @ViewBuilder
    private func infoChip(_ info: String) -> some View {
        Button(action: { showingInfo.toggle() }) {
            HStack(spacing: 5) {
                Image(systemName: showingInfo ? "info.circle.fill" : "info.circle")
                    .font(.system(size: 12, weight: .regular))
                Text("cos'è?")
                    .font(.mono(10))
                    .tracking(0.5)
            }
            .foregroundStyle(hovering || showingInfo ? Tokens.text : Tokens.textFaint)
            .padding(.horizontal, 9).padding(.vertical, 4)
            .background(
                Capsule().fill(hovering ? Color.white.opacity(0.05) : Color.clear)
            )
            .overlay(
                Capsule().strokeBorder(
                    hovering || showingInfo ? Tokens.textGhost : Color.clear,
                    lineWidth: 1
                )
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isIn in
            hovering = isIn
            #if canImport(AppKit)
            if isIn { NSCursor.pointingHand.push() } else { NSCursor.pop() }
            #endif
        }
        .accessibilityLabel("Maggiori informazioni su \(title)")
        .accessibilityHint(info)
        .popover(isPresented: $showingInfo, arrowEdge: .bottom) {
            infoPopover(info)
        }
    }

    private func infoPopover(_ info: String) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 6) {
                Text(kicker)
                    .font(.mono(9)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("·")
                    .foregroundStyle(Tokens.textFaint)
                Text(title.lowercased())
                    .font(.serif(13, italic: true))
                    .foregroundStyle(Tokens.textDim)
            }
            Text(info)
                .font(.serif(14))
                .foregroundStyle(Tokens.text)
                .lineSpacing(4)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(16)
        .frame(width: 360, alignment: .leading)
        .background(Tokens.bg2)
    }
}

// MARK: — macOS traffic lights

public struct TrafficLights: View {
    public var body: some View {
        HStack(spacing: 8) {
            ForEach(
                [Color(hex: 0xFF5F57), Color(hex: 0xFEBC2E), Color(hex: 0x28C840)],
                id: \.self
            ) { c in
                Circle()
                    .fill(c)
                    .frame(width: 12, height: 12)
                    .overlay(Circle().strokeBorder(.black.opacity(0.25), lineWidth: 0.5))
            }
        }
    }
}
