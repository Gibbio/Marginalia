import SwiftUI

/// Second onboarding step: list which models are still missing and let the
/// user trigger downloads. Works against any `MarginaliaHost` — live and
/// mock both expose `inflightDownloads: [String: InstallUiState]` and
/// `installAsset(_:)`. Missing key in the dict means "idle" (not installed,
/// not inflight).
public struct InstallModelsView: View {
    public struct Asset: Identifiable, Hashable {
        public let id: String
        public let label: String
        public let size: String
        public let installed: Bool
        public init(id: String, label: String, size: String, installed: Bool) {
            self.id = id; self.label = label; self.size = size; self.installed = installed
        }
    }

    public var accent: Accent
    public var assets: [Asset]
    /// Per-asset transient state, keyed by asset id. Missing keys = idle.
    public var inflightStates: [String: InstallUiState]
    public var onInstall: (String) -> Void
    /// Optional — when provided, installed rows show a "rimuovi" button.
    /// Nil (the default) hides it, keeping the view compatible with flows
    /// that don't support uninstall (preview / mock).
    public var onUninstall: ((String) -> Void)?
    public var onProceed: () -> Void
    public var onSkip: () -> Void

    public init(
        accent: Accent,
        assets: [Asset],
        inflightStates: [String: InstallUiState] = [:],
        onInstall: @escaping (String) -> Void = { _ in },
        onUninstall: ((String) -> Void)? = nil,
        onProceed: @escaping () -> Void,
        onSkip: @escaping () -> Void
    ) {
        self.accent = accent
        self.assets = assets
        self.inflightStates = inflightStates
        self.onInstall = onInstall
        self.onUninstall = onUninstall
        self.onProceed = onProceed
        self.onSkip = onSkip
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            VStack(alignment: .leading, spacing: 8) {
                Text("INSTALLAZIONE")
                    .font(.mono(10)).tracking(2)
                    .foregroundStyle(Tokens.textFaint)
                Text("Un momento: servono alcuni modelli")
                    .font(.serif(32, weight: .medium))
                    .kerning(-0.3)
                    .foregroundStyle(Tokens.text)
                Text("Questi file non sono inclusi nel pacchetto per tenerla leggera. Li scarichi una volta, funzionano per sempre offline.")
                    .font(.serif(15, italic: true))
                    .foregroundStyle(Tokens.textDim)
                    .lineSpacing(4)
                    .frame(maxWidth: 620, alignment: .leading)
            }

            VStack(spacing: 0) {
                ForEach(Array(assets.enumerated()), id: \.element.id) { idx, asset in
                    assetRow(asset)
                    if idx < assets.count - 1 {
                        Divider().frame(height: 1).overlay(Tokens.lineSoft)
                    }
                }
            }
            .overlay(
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .strokeBorder(Tokens.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))

            networkBanner

            HStack(spacing: 12) {
                Button(action: installAllMissing) {
                    Text(hasMissing ? "scarica" : "tutto pronto")
                        .font(.sans(14, weight: .medium))
                        .foregroundStyle(hasMissing ? Tokens.bg : Tokens.textDim)
                        .padding(.horizontal, 22).padding(.vertical, 9)
                        .background(
                            Capsule().fill(hasMissing ? accent.main : Color.white.opacity(0.04))
                        )
                        .overlay(
                            Capsule().strokeBorder(hasMissing ? accent.main : Tokens.line)
                        )
                }
                .buttonStyle(.plain)
                .disabled(!hasMissing || anyInflightActive)

                if !hasMissing {
                    Button(action: onProceed) {
                        Text("continua")
                            .font(.sans(14, weight: .medium))
                            .foregroundStyle(Tokens.bg)
                            .padding(.horizontal, 22).padding(.vertical, 9)
                            .background(Capsule().fill(accent.main))
                    }
                    .buttonStyle(.plain)
                }

                Button(action: onSkip) {
                    Text("salta per ora")
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
    }

    private var hasMissing: Bool {
        assets.contains { !isEffectivelyInstalled($0) }
    }

    private var anyInflightActive: Bool {
        inflightStates.values.contains { $0.isInflight }
    }

    private func isEffectivelyInstalled(_ asset: Asset) -> Bool {
        if asset.installed { return true }
        return inflightStates[asset.id] == .installed
    }

    private func installAllMissing() {
        for a in assets where !isEffectivelyInstalled(a) {
            onInstall(a.id)
        }
    }

    @ViewBuilder
    private func assetRow(_ asset: Asset) -> some View {
        let state = rowState(for: asset)
        HStack(spacing: 14) {
            stateDot(for: state)

            VStack(alignment: .leading, spacing: 2) {
                Text(asset.label)
                    .font(.serif(15, italic: state == .installed))
                    .foregroundStyle(Tokens.text)
                Text(secondaryLine(size: asset.size, state: state))
                    .font(.mono(10))
                    .foregroundStyle(state == .failed("") ? Color.red.opacity(0.85) : Tokens.textFaint)
            }
            Spacer()
            trailingControl(for: asset, state: state)
        }
        .padding(.horizontal, 16).padding(.vertical, 14)
    }

    /// Internal state used by the dot / trailing control. Derived from the
    /// asset's `installed` flag plus the transient `inflightStates` dict.
    /// `downloading(fraction:)` carries 0.0–1.0 when hf-hub has reported
    /// the total; nil = first frame before Content-Length negotiation.
    private enum RowState: Equatable {
        case idle, queued
        case downloading(fraction: Double?)
        case installed, failed(String)
    }

    private func rowState(for asset: Asset) -> RowState {
        if asset.installed { return .installed }
        switch inflightStates[asset.id] {
        case .queued?:                    return .queued
        case .downloading(let f)?:        return .downloading(fraction: f)
        case .installed?:                 return .installed
        case .failed(let m)?:             return .failed(m)
        case nil:                         return .idle
        }
    }

    @ViewBuilder
    private func stateDot(for state: RowState) -> some View {
        ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(fillColor(for: state))
            RoundedRectangle(cornerRadius: 6)
                .strokeBorder(strokeColor(for: state), lineWidth: 1)
            stateGlyph(for: state)
        }
        .frame(width: 26, height: 26)
    }

    @ViewBuilder
    private func stateGlyph(for state: RowState) -> some View {
        switch state {
        case .installed:
            Image(systemName: "checkmark")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(accent.main)
        case .downloading:
            // Circular spinner in the 26 px dot — the determinate bar is
            // too small to read at this size. The text subtitle carries
            // the percentage instead.
            ProgressView()
                .progressViewStyle(.circular)
                .controlSize(.small)
                .tint(accent.main)
        case .queued:
            Image(systemName: "clock")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(Tokens.textFaint)
        case .failed:
            Image(systemName: "exclamationmark")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.red.opacity(0.85))
        case .idle:
            Image(systemName: "arrow.down")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Tokens.textFaint)
        }
    }

    private func fillColor(for state: RowState) -> Color {
        switch state {
        case .installed:    return accent.main.opacity(0.15)
        case .failed:       return Color.red.opacity(0.08)
        case .downloading:  return accent.main.opacity(0.05)
        default:            return Color.white.opacity(0.03)
        }
    }

    private func strokeColor(for state: RowState) -> Color {
        switch state {
        case .installed:    return accent.main.opacity(0.35)
        case .failed:       return Color.red.opacity(0.4)
        case .downloading:  return accent.main.opacity(0.25)
        default:            return Tokens.line
        }
    }

    private func secondaryLine(size: String, state: RowState) -> String {
        switch state {
        case .installed:    return "\(size) · installato"
        case .queued:       return "\(size) · in coda"
        case .downloading(let f):
            if let f = f { return "\(size) · \(Int(f * 100))%" }
            return "\(size) · scaricando…"
        case .failed(let msg):
            return "errore: \(msg.isEmpty ? "non disponibile" : msg)"
        case .idle:         return "\(size) · da scaricare"
        }
    }

    @ViewBuilder
    private func trailingControl(for asset: Asset, state: RowState) -> some View {
        switch state {
        case .installed:
            if let onUninstall = onUninstall {
                Button("rimuovi") { onUninstall(asset.id) }
                    .buttonStyle(.plain)
                    .font(.mono(11))
                    .foregroundStyle(Tokens.textDim)
                    .padding(.horizontal, 10).padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 5)
                        .fill(Color.white.opacity(0.04)))
                    .overlay(RoundedRectangle(cornerRadius: 5)
                        .strokeBorder(Tokens.line, lineWidth: 1))
            } else {
                EmptyView()
            }
        case .queued:
            EmptyView()
        case .downloading(let f):
            // Linear bar here, visible next to the label, so the user
            // watches bytes move even when the dot's spinner blurs.
            if let f = f {
                ProgressView(value: f, total: 1.0)
                    .progressViewStyle(.linear)
                    .tint(accent.main)
                    .frame(width: 100)
            } else {
                EmptyView()
            }
        case .failed:
            Button("riprova") { onInstall(asset.id) }
                .buttonStyle(.plain)
                .font(.mono(11))
                .foregroundStyle(accent.main)
                .padding(.horizontal, 10).padding(.vertical, 4)
                .background(RoundedRectangle(cornerRadius: 5)
                    .fill(accent.main.opacity(0.08)))
                .overlay(RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(accent.main.opacity(0.3), lineWidth: 1))
        case .idle:
            Button("installa") { onInstall(asset.id) }
                .buttonStyle(.plain)
                .font(.mono(11))
                .foregroundStyle(accent.main)
                .padding(.horizontal, 10).padding(.vertical, 4)
                .background(RoundedRectangle(cornerRadius: 5)
                    .fill(accent.main.opacity(0.08)))
                .overlay(RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(accent.main.opacity(0.3), lineWidth: 1))
        }
    }

    private var networkBanner: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "network")
                .font(.system(size: 11))
                .foregroundStyle(Tokens.textFaint)
                .padding(.top, 2)
            VStack(alignment: .leading, spacing: 3) {
                Text("L'UNICO MOMENTO IN CUI SI USA LA RETE")
                    .font(.mono(10)).tracking(1.5)
                    .foregroundStyle(Tokens.textFaint)
                Text("I file arrivano da huggingface.co. Dopo il download, Marginalia funziona completamente offline — niente altri collegamenti di rete.")
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
}
