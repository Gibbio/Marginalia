#if canImport(AppKit)
import AppKit

/// Keeps preview `NSSound` instances alive for the duration of playback.
/// `SettingsView` is generic over the host type and can't have a static
/// stored property of its own, so this lives outside. Singleton keyed on
/// the main actor since NSSound playback is not thread-safe.
@MainActor
public final class PreviewSoundCache {
    public static let shared = PreviewSoundCache()
    private var sounds: [NSSound] = []

    public func add(_ sound: NSSound) {
        sounds.append(sound)
        // Evict after a generous timeout (preview clips are a few seconds).
        Task.detached { [weak self] in
            try? await Task.sleep(for: .seconds(30))
            await MainActor.run {
                self?.sounds.removeAll { $0 === sound }
            }
        }
    }

    /// Load and play a sound file from disk. Used by Settings voice preview
    /// and by the onboarding flow's "ecco Sara" moment that auto-plays the
    /// first installed voice so the user immediately *hears* the app work.
    public static func play(path: String) {
        guard let sound = NSSound(contentsOfFile: path, byReference: false) else { return }
        shared.add(sound)
        sound.play()
    }
}
#endif
