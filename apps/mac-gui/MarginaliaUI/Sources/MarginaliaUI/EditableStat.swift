import SwiftUI

/// Editable numeric stat card used in the STT tuning grid. Replaces the
/// read-only `MiniStat` where the value is a configurable timeout /
/// duration. Supports both typed input and +/- stepper buttons.
public struct EditableStat: View {
    public var label: String
    public var unit: String
    @Binding public var value: Double
    public var range: ClosedRange<Double>
    public var step: Double
    /// How the numeric value is rendered. Default: one decimal.
    public var format: String = "%.1f"

    @State private var draftText: String = ""
    @FocusState private var focused: Bool

    public init(label: String,
                unit: String = "s",
                value: Binding<Double>,
                range: ClosedRange<Double>,
                step: Double = 0.1,
                format: String = "%.1f") {
        self.label = label
        self.unit = unit
        self._value = value
        self.range = range
        self.step = step
        self.format = format
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label.uppercased())
                .font(.mono(10))
                .tracking(1.2)
                .foregroundStyle(Tokens.textFaint)

            HStack(alignment: .firstTextBaseline, spacing: 2) {
                // Editable numeric field. Flashes red if out-of-range.
                TextField("", text: $draftText)
                    .textFieldStyle(.plain)
                    .font(.serif(20))
                    .foregroundStyle(Tokens.text)
                    .focused($focused)
                    .onSubmit(commit)
                    .onChange(of: focused) { _, now in
                        if !now { commit() }
                    }
                    .fixedSize(horizontal: true, vertical: false)
                    .multilineTextAlignment(.trailing)

                // Unit sits flush-right of the number, same weight-family
                // as the value so the pair reads "1.5s" as one glyph group.
                Text(unit)
                    .font(.serif(20))
                    .foregroundStyle(Tokens.textDim)

                Spacer(minLength: 6)

                VStack(spacing: 0) {
                    Button(action: { bump(+step) }) {
                        Image(systemName: "chevron.up")
                            .font(.system(size: 9, weight: .semibold))
                            .foregroundStyle(Tokens.textDim)
                            .frame(width: 22, height: 14)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Aumenta \(label)")

                    Button(action: { bump(-step) }) {
                        Image(systemName: "chevron.down")
                            .font(.system(size: 9, weight: .semibold))
                            .foregroundStyle(Tokens.textDim)
                            .frame(width: 22, height: 14)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Diminuisci \(label)")
                }
                .overlay(
                    RoundedRectangle(cornerRadius: 4)
                        .strokeBorder(Tokens.line, lineWidth: 1)
                )
            }
        }
        .padding(.horizontal, 14).padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(Color.white.opacity(0.02))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .strokeBorder(Tokens.line, lineWidth: 1)
        )
        .onAppear { draftText = String(format: format, value) }
        .onChange(of: value) { _, new in
            // Sync external changes into the field (e.g. reset to default).
            draftText = String(format: format, new)
        }
    }

    private func bump(_ delta: Double) {
        value = clamp(value + delta)
        draftText = String(format: format, value)
    }

    private func commit() {
        // Italian locale often uses "," as decimal — accept both.
        let normalized = draftText.replacingOccurrences(of: ",", with: ".")
        if let parsed = Double(normalized) {
            value = clamp(parsed)
        }
        draftText = String(format: format, value)
    }

    private func clamp(_ v: Double) -> Double {
        min(range.upperBound, max(range.lowerBound, v))
    }
}
