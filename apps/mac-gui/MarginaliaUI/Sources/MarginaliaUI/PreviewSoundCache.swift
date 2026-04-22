#if canImport(AppKit)
import AppKit

/// Keeps preview `NSSound` instances alive for the duration of playback.
/// `SettingsView` is generic over the host type and can't have a static
/// stored property of its own, so this lives outside. Singleton keyed on
/// the main actor since NSSound playback is not thread-safe.
@MainActor
final class PreviewSoundCache {
    static let shared = PreviewSoundCache()
    private var sounds: [NSSound] = []

    func add(_ sound: NSSound) {
        sounds.append(sound)
        // Evict after a generous timeout (preview clips are a few seconds).
        Task.detached { [weak self] in
            try? await Task.sleep(for: .seconds(30))
            await MainActor.run {
                self?.sounds.removeAll { $0 === sound }
            }
        }
    }
}
#endif
