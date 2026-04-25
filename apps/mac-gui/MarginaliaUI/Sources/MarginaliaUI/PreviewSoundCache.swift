#if canImport(AppKit)
import AppKit
import AVFoundation

/// Keeps preview sound instances alive for the duration of playback.
/// `SettingsView` is generic over the host type and can't have a static
/// stored property of its own, so this lives outside. Singleton keyed on
/// the main actor since playback APIs are not thread-safe.
@MainActor
public final class PreviewSoundCache {
    public static let shared = PreviewSoundCache()
    private var sounds: [NSSound] = []
    private var players: [AVAudioPlayer] = []

    public func add(_ sound: NSSound) {
        sounds.append(sound)
        Task.detached { [weak self] in
            try? await Task.sleep(for: .seconds(30))
            await MainActor.run {
                self?.sounds.removeAll { $0 === sound }
            }
        }
    }

    public func add(_ player: AVAudioPlayer) {
        players.append(player)
        let token = ObjectIdentifier(player)
        Task.detached { [weak self] in
            try? await Task.sleep(for: .seconds(60))
            await MainActor.run {
                self?.players.removeAll { ObjectIdentifier($0) == token }
            }
        }
    }

    /// Load and play a sound file from disk. Tries NSSound first (handles
    /// WAV/AIFF/MP3 out of the box); falls back to AVAudioPlayer for
    /// formats NSSound can't decode on this macOS version — in practice
    /// FLAC, which the TTS backend emits for synthesized previews and
    /// note playback. Silent no-op only if both paths fail.
    public static func play(path: String) {
        if let sound = NSSound(contentsOfFile: path, byReference: false) {
            shared.add(sound)
            sound.play()
            return
        }
        let url = URL(fileURLWithPath: path)
        if let player = try? AVAudioPlayer(contentsOf: url) {
            player.prepareToPlay()
            shared.add(player)
            player.play()
            return
        }
        NSLog("[PreviewSoundCache] could not load sound at \(path)")
    }

    /// Halt every sound currently in flight and drop the cached
    /// instances. Called when the user deletes a note that's currently
    /// being played back so the audio doesn't keep going past the row's
    /// disappearance.
    public static func stopAll() {
        for s in shared.sounds { s.stop() }
        shared.sounds.removeAll()
        for p in shared.players { p.stop() }
        shared.players.removeAll()
    }
}
#endif
