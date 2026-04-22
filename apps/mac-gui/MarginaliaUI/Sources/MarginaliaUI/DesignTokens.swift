import SwiftUI

/// Design tokens mirroring the `d` object in `desktop-app.jsx` (Inchiostro palette).
///
/// Keep token names aligned with the JSX prototype so design handoff changes
/// are trivial to port: if the designer edits `d.textDim` in the HTML, we edit
/// `Tokens.textDim` here.
public enum Tokens {
    // Backgrounds (warm vellum-dark).
    public static let bg        = Color(hex: 0x0F0E10)
    public static let bg2       = Color(hex: 0x161315)
    public static let paper     = Color(hex: 0x1A1815)

    // Foregrounds on the bg (oklch roughly 0.91 0.05 80).
    public static let text      = Color(hex: 0xEFE5CF)
    public static let textDim   = Color(hex: 0xEFE5CF, opacity: 0.58)
    public static let textFaint = Color(hex: 0xEFE5CF, opacity: 0.28)
    public static let textGhost = Color(hex: 0xEFE5CF, opacity: 0.14)

    // Hairlines.
    public static let line      = Color(hex: 0xEFE5CF, opacity: 0.08)
    public static let lineSoft  = Color(hex: 0xEFE5CF, opacity: 0.04)

    // Font families: falls back to system fonts if the named one isn't embedded.
    public static let serifName   = "Cormorant Garamond"
    public static let serifFallback: Font.Design = .serif
    public static let sansName    = "Inter Tight"
    public static let sansFallback: Font.Design = .default
    public static let monoName    = "JetBrains Mono"
    public static let monoFallback: Font.Design = .monospaced
}

/// Accent colour family driven by a single hue parameter (matches JSX `makeAccent`).
/// SwiftUI has no OKLCH, so we approximate with HSB values chosen to match the
/// perceptual mid-blue Inchiostro target.
public struct Accent: Equatable, Sendable {
    public let hue: Double   // 200 … 310 (degrees)
    public let main: Color
    public let deep: Color
    public let soft: Color   // 18% alpha fill
    public let glow: Color   // 32% alpha fill (use inside shadows)

    public init(hue: Double) {
        self.hue = hue
        let h = hue / 360.0  // SwiftUI expects 0…1
        // Keep saturation moderate (0.46) and brightness high (0.85) — that
        // reads as "bright ink" on the warm-dark bg, matching the prototype.
        self.main = Color(hue: h, saturation: 0.46, brightness: 0.85)
        self.deep = Color(hue: h, saturation: 0.55, brightness: 0.65)
        self.soft = Color(hue: h, saturation: 0.46, brightness: 0.85).opacity(0.18)
        self.glow = Color(hue: h, saturation: 0.46, brightness: 0.85).opacity(0.32)
    }

    public static let `default` = Accent(hue: 250)
}

// MARK: — Themes

/// A named visual theme. For now the backgrounds stay the same warm-dark
/// vellum (core identity of Marginalia); themes differ in their accent
/// hue, which cascades through every highlight, picker selection, dock
/// pill, waveform colour, and focus ring. Future themes could shift the
/// paper tone too — but the current single-token approach keeps the app
/// coherent without adding a heavy theming layer.
public struct Theme: Hashable, Codable, Sendable {
    public let id: String
    /// Italian-first display name. The English equivalent lives in
    /// Localizable.strings under `theme.<id>`.
    public let display: String
    public let blurb: String
    public let accentHue: Double
}

public enum AppTheme {
    /// Special id used by the "custom" theme — the accent hue is then
    /// read from `customHueKey` instead of the preset table.
    public static let customId = "custom"
    public static let customHueKey = "com.gibbio.marginalia.customAccentHue"

    /// Available themes in display order. "Custom" is a placeholder row
    /// with a placeholder hue; the actual value comes from AppStorage at
    /// resolve time.
    public static let all: [Theme] = [
        Theme(id: "inchiostro",
              display: "Inchiostro",
              blurb: "il tema originale, con un accento indaco/viola — pensato come inchiostro fresco su vellum.",
              accentHue: 250),
        Theme(id: "marea",
              display: "Marea",
              blurb: "un blu più calmo e freddo, meno saturato — per le sessioni di lettura lunghe.",
              accentHue: 208),
        Theme(id: AppTheme.customId,
              display: "Personalizzata",
              blurb: "scegli la tinta di accento con lo slider qui sotto.",
              accentHue: 30),
    ]

    public static let `default` = all[0]
    public static let storageKey = "com.gibbio.marginalia.theme"

    /// Resolve an `Accent` palette from a stored theme id. For the custom
    /// theme, reads the user's stored hue from `UserDefaults`; falls back
    /// to 30° (warm orange) if no value is stored.
    public static func accent(for id: String) -> Accent {
        if id == customId {
            let hue = UserDefaults.standard.object(forKey: customHueKey) as? Double
                ?? 30
            return Accent(hue: hue)
        }
        return Accent(hue: (all.first { $0.id == id } ?? `default`).accentHue)
    }
}

// MARK: — Color hex convenience

extension Color {
    /// Creates a Color from a 0xRRGGBB integer literal. Matches the `#rrggbb`
    /// values used in the JSX design tokens.
    public init(hex: Int, opacity: Double = 1.0) {
        let r = Double((hex >> 16) & 0xFF) / 255.0
        let g = Double((hex >>  8) & 0xFF) / 255.0
        let b = Double( hex        & 0xFF) / 255.0
        self.init(.sRGB, red: r, green: g, blue: b, opacity: opacity)
    }
}

// MARK: — Font convenience

public extension Font {
    /// Cormorant Garamond (variable font), optional italic.
    ///
    /// We don't pass `.weight(...)` on top of a custom variable font — SwiftUI
    /// emits a spammy "Unable to update Font Descriptor's weight" warning for
    /// every call, because variable fonts respond to the `wght` axis rather
    /// than discrete faces. For now all text uses the default axis value
    /// (≈400/regular). Add a static `CormorantGaramond-Medium.ttf` and a
    /// second helper if a heavier face becomes necessary for the design.
    static func serif(_ size: CGFloat, weight: Font.Weight = .regular,
                      italic: Bool = false) -> Font {
        _ = weight  // accepted for API symmetry; unused for the reason above
        let f = Font.custom(Tokens.serifName, size: size)
        return italic ? f.italic() : f
    }

    /// Inter Tight.
    static func sans(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        _ = weight
        return Font.custom(Tokens.sansName, size: size)
    }

    /// JetBrains Mono.
    static func mono(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        _ = weight
        return Font.custom(Tokens.monoName, size: size)
    }
}
