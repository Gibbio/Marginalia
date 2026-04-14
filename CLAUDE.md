# Marginalia — AI Agent Context

Marginalia is a local-first voice reading and annotation engine written in Rust.
It ingests text/markdown documents, chunks them, synthesizes speech via TTS, and
lets the user navigate, annotate, and interact with the content by voice.

## Repository layout

```
apps/
  tui-rs/           Terminal UI — the main desktop interface
  cli-rs/           CLI for testing core (ingest, read, bench)

crates/
  marginalia-core/          Domain types, ports (traits), application services
  marginalia-runtime/       Orchestrates core + storage + providers
  marginalia-storage-sqlite/ SQLite persistence
  marginalia-import-text/   Text/Markdown importer with chunking
  marginalia-import-pdf/    PDF importer via PDFium (requires models/pdf/lib)
  marginalia-import-epub/   EPUB 2/3 importer via `epub` crate (pure Rust)
  marginalia-tts-mlx/       Kokoro TTS via MLX Metal GPU (macOS Apple Silicon)
  marginalia-tts-kokoro/    Kokoro TTS via ONNX Runtime (cross-platform fallback)
  marginalia-stt-apple/     Apple SFSpeechRecognizer via Swift helper (macOS)
  marginalia-stt-whisper/   Whisper STT (optional, needs cmake)
  marginalia-stt-vosk/      Vosk STT (legacy, no longer wired in tui-rs)
  marginalia-playback-host/ Audio playback (rodio in-process)
  marginalia-provider-fake/ Fake providers for testing
  marginalia-devtools/      Dev utilities

models/              Model asset layout (downloaded via make bootstrap-beta)
benchmark/           TTS backend benchmark suite
```

## Architecture

Hexagonal architecture. Core has zero external dependencies — all I/O through traits (ports).

Key traits in `marginalia-core::ports`:
- `SpeechSynthesizer` — TTS (implemented by marginalia-tts-mlx and marginalia-tts-kokoro)
- `DocumentRepository`, `SessionRepository`, `NoteRepository` — storage
- `DocumentImporter` — file import
- `PlaybackEngine` — audio playback
- `CommandRecognizer`, `DictationTranscriber` — STT

The runtime (`marginalia-runtime`) composes everything. Apps create providers
and pass them to the runtime via `set_speech_synthesizer()`, etc.

## TTS — the critical path

TTS performance is the main UX constraint. Current backends:

| Crate | Backend | Platform | ~Latency (164ch) |
|---|---|---|---|
| marginalia-tts-mlx | Kokoro-82M via MLX Metal | macOS Apple Silicon | ~1000ms |
| marginalia-tts-kokoro | Kokoro-82M via ONNX Runtime | Cross-platform | ~5700ms |

`make tui-rs` auto-selects MLX on macOS arm64. Configuration in `apps/tui-rs/marginalia.toml`:

```toml
[mlx]
voice = "if_sara"    # Italian female (or im_nicola for male)
```

MLX deps come from:
- `Gibbio/voice-mlx` (GitHub) — forked voice-tts/voice-nn/voice-dsp with patched decoder
- `oxideai/mlx-rs` (GitHub, git HEAD) — Rust MLX bindings (must use git, not crates.io v0.25.3 which bundles old MLX C++)

### Async navigation + cache + prefetch

Three mechanisms work together to minimize latency:

1. **Async commands**: navigation commands (next/back/repeat etc.) run in a
   background thread so the UI never freezes during TTS synthesis.

2. **TTS cache**: synthesized chunks are cached by (document_id, section, chunk,
   voice). WAV files persist on disk in `.marginalia-tts-cache/`. Revisiting a
   chunk is instant.

3. **Background prefetch**: after a navigation command completes, a **separate**
   fire-and-forget thread pre-synthesizes the next chunk into the cache. The
   prefetch thread acquires the runtime lock only after the command thread
   releases it, so the UI stays responsive and playback starts immediately.

Result: first chunk ~1s, all subsequent chunks instant in sequential reading.

**IMPORTANT**: prefetch must NEVER run in the same thread as the current command.
A previous attempt did this and froze the UI for ~2s (current chunk + prefetch).
The fix was to spawn a separate `std::thread` from `poll_async_result` after
the command succeeds, not from `replay_session_at_position`.

## Phonemizer — misaki reference

The phonemizer in `marginalia-tts-mlx` replicates the behavior of
[misaki](https://github.com/hexgrad/misaki) (`misaki/espeak.py`), the official
G2P engine for Kokoro, written by the model author (hexgrad).

How it works:
1. **Normalize** text: `() [] {}` → commas, dashes → commas, smart quotes → ASCII
2. **Split** on clause punctuation (`. , ! ? : ;` and CJK equivalents)
3. **Phonemize** each clause via `espeak-ng --ipa`
4. **Re-insert** punctuation between phoneme clauses
5. **Clean** IPA output: remove tie chars (`^`), combining diacritics (U+0329, U+032A)

This is language-agnostic — works for any language espeak-ng supports.
Adding a new language requires zero source changes, just a new voice in the toml.

**Reference source**: `hexgrad/misaki/misaki/espeak.py` — `EspeakG2P.__call__`
and `EspeakFallback.__call__`. When in doubt about phonemizer behavior, check
misaki first. It is the canonical reference for how Kokoro expects phonemes.

## Build

```bash
cargo build --release                    # default (no native deps)
cargo build --release -p marginalia-tui --features mlx-tts   # with MLX TTS
make tui-rs                              # auto-detects platform
cargo test                               # all tests
```

Building marginalia-tts-mlx requires Xcode + Metal Toolchain on macOS.

## STT — speech-to-text

ONE engine handles BOTH command recognition AND note dictation, with per-context
tuning. The engine is selected in `[stt] engine = "apple" | "whisper"`.

| Engine | Latency | Dictation | Requires |
|---|---|---|---|
| **Apple** (macOS) | ~0.2-0.3s | Yes (same Swift helper, mode-switch) | macOS Dictation enabled |
| **Whisper** | ~2s | Yes (separate WhisperConfig profile) | Whisper ggml model (~460MB) |

### Config layout (final, do not restructure)

```toml
[voice_commands]        # trigger words → actions
pause = ["pausa", ...]
# ...

[stt]                   # engine selection + shared options
engine   = "apple"      # "apple" or "whisper"
language = "it"         # ISO ("it") or BCP-47 ("it-IT"); auto-converted per engine
debug    = true

[stt.apple]             # apple-engine settings (placeholder for future options)

[stt.whisper]           # whisper-engine settings
model_path = "models/stt/whisper/ggml-small.bin"

[stt.commands]          # tuning profile for SHORT utterances
silence_timeout    = 0.8
max_record_seconds = 4
speech_threshold   = 500

[stt.dictation]         # tuning profile for LONG utterances (/note)
silence_timeout    = 1.5
max_record_seconds = 60
speech_threshold   = 500
```

### Apple STT — architecture (AEC3 + Swift helper)

The Apple STT pipeline has three layers:

```
WAV chunk ──callback──→ AEC3 (render reference)
                              │
Mic (cpal) ──resample 24k──→ AEC3 (capture) ──cleaned──→ Swift helper (TLV stdin)
                                                              │
                                                     SFSpeechRecognizer
                                                              │
                                                     CMD/DICT_END (stdout)
                                                              │
                                                     Rust reader thread
```

**Layer 1 — Acoustic echo cancellation** (`aec_pipeline.rs`):
Rust captures the mic via `cpal`, resamples to 24kHz mono, and feeds each
10ms frame through `aec3::voip::VoipAec3` (pure-Rust port of WebRTC AEC3).
The render reference (what the TTS is playing) comes from a callback on
`HostPlaybackEngine`: when a chunk starts playing, the WAV samples are sent
to the AEC thread, which advances through them frame-by-frame in lockstep
with the mic capture. AEC3 subtracts the reference from the mic → the Swift
helper never sees the TTS echo.

**Layer 2 — Swift helper** (`SWIFT_HELPER_SOURCE`, compiled to `stt-helper-vN`):
No longer owns the mic (no AVAudioEngine, no installTap). Reads cleaned
audio from stdin via a binary TLV protocol:
- Type `0x41` ('A'): audio frame (f32 samples, little-endian, 10ms @ 24kHz)
- Type `0x4D` ('M'): mode command ("COMMAND" or "DICTATION")

Wraps audio frames in `AVAudioPCMBuffer` and feeds them to
`SFSpeechAudioBufferRecognitionRequest`. Emits recognized text to stdout
with prefixes:
- `CMD <text>` — command mode utterance
- `DICT_END <text>` — finalized dictation

Mode switching: triggers `scheduleRestart()` which ends the current
recognition task and starts a fresh one with the new mode's silence timer.

**Layer 3 — Rust consumers** (same as before):
Both `AppleCommandRecognizer` (commands) and `AppleDictationTranscriber`
(notes) share the helper process via `Arc<AppleHelperShared>`. A single
reader thread routes stdout lines to two mpsc channels based on prefix.

Constructor returns all three — do not build them separately:
```rust
let (recognizer, dict_transcriber, aec_pipeline) = new_apple_stt(
    language, commands, cmd_silence, dict_silence, dict_max
)?;
runtime.set_command_recognizer(recognizer);
runtime.set_dictation_transcriber(dict_transcriber);
// aec_pipeline is Box::leaked to keep the cpal stream alive for the process
```

**No trigger fast-path**: all emissions go through the silence timer
(configurable via `[stt.commands] silence_timeout`). This was changed when
AEC3 was added — the fast-path caused multi-word triggers like "prossimo
capitolo" to fire prematurely (the first word "prossimo" matched the
single-word trigger "next" before the user could say "capitolo").

### Apple STT — audio feedback on mode change

Helper plays a Tink on `command → dictation` and a Pop on `dictation → command`,
via `AudioServicesPlaySystemSound` (AudioToolbox, no AppKit). Loaded once at
startup from `/System/Library/Sounds/{Tink,Pop}.aiff`. No-op if loading fails.
Conditional on the actual transition (skips beep on redundant MODE writes).

### Apple STT — HELPER_VERSION

`crates/marginalia-stt-apple/src/lib.rs` embeds the Swift source as a string
constant. The compiled binary is cached at `$TMPDIR/marginalia-stt-apple/stt-helper-vN`.
**Bump `HELPER_VERSION` whenever you change `SWIFT_HELPER_SOURCE`**, otherwise
users will silently run the old cached binary. The constant is near the top of
`lib.rs` for easy maintenance.

### Whisper STT — two WhisperConfigs

When `engine = "whisper"`, backend.rs builds TWO `WhisperConfig` instances from
the same `[stt.whisper] model_path`: one for commands (short defaults: 4s max,
0.8s silence) and one for dictation (long defaults: 60s max, 1.5s silence).
`[stt.commands]` and `[stt.dictation]` override their respective profiles
independently.

### The mic stays open

Both engines keep the microphone stream open for the entire session
(no permission icon flickering). Command monitor thread runs independently
from the runtime, so there's no lock contention while navigating or replaying.

### Echo cancellation — acoustic AEC3

Echo cancellation prevents the TTS from triggering its own voice commands
when the mic picks up the playback audio.

On macOS, the `aec3` crate (pure-Rust port of WebRTC AEC3) removes the TTS
audio from the mic signal BEFORE it reaches SFSpeechRecognizer. See "Apple
STT — architecture" above. The render reference is the WAV chunk being
played; the capture is the resampled mic. AEC3 subtracts one from the other
per 10ms frame. This handles echo with zero false negatives.

For **other platforms**, the AEC approach will differ:
- **iOS**: `AVAudioSession.voiceChat` (hardware AEC on Neural Engine)
- **Linux**: PipeWire `echo-cancel` module or `aec3` crate
- **Windows**: Communications APO or `aec3` crate
- **Android**: `AcousticEchoCanceler` framework class
- **Web**: `getUserMedia({echoCancellation: true})`

The per-platform evaluation matrix lives in NEXT.md.

## Key conventions

- Italian is the primary language (documents, TTS voices, STT commands)
- Chunk target: ~300 characters per chunk
- Audio: 24kHz sample rate (Kokoro), 22050Hz (Piper)
- Config: TOML (`apps/tui-rs/marginalia.toml`), generated from template by `make tui-rs`
- Default voice: `af_bella` (English), `if_sara` / `im_nicola` (Italian)
- espeak-ng is used as external phonemizer (all languages, clause-by-clause)
- Phonemizer rules follow misaki (hexgrad/misaki) — the Kokoro reference G2P

## What NOT to do

- Don't add Python dependencies to the main codebase
- Don't use CoreML EP with Kokoro (dynamic shapes → deadlock/crash)
- Don't use mlx-rs from crates.io (v0.25.3 bundles MLX C++ 0.25, too slow)
- Don't make TTS calls synchronous in the UI thread
- Don't add unnecessary abstractions — three similar lines > premature abstraction
- Don't invent phonemizer rules — check misaki (hexgrad/misaki) first
- Don't call espeak-ng on full text (strips punctuation) — call per clause
- Don't open/close audio streams per capture cycle — keep them persistent
- Don't change core traits without checking all implementations compile
- Don't add a new provider without at least a basic smoke test
- Don't restructure the `marginalia.toml` schema — the layout in the STT
  section above is stable. Add fields inside existing sections; don't create
  new top-level roots.
- Don't spawn multiple Swift helper processes for Apple STT. The single
  shared process with mode-switch-via-stdin is the deliberate architecture.
  Extend `SWIFT_HELPER_SOURCE` instead of spawning another helper.
- Don't let the Swift helper capture the mic — it reads AEC-cleaned audio
  from stdin via TLV binary. The mic is owned by cpal on the Rust side.
  No `AVAudioEngine`, no `installTap`, no `audioEngine.start()` in the helper.
- Don't re-add a trigger fast-path to the Swift helper. It was removed
  because it fired multi-word triggers prematurely (e.g. "prossimo"
  matching before the user could say "capitolo"). With AEC3 handling echo,
  the silence timer alone (0.8s default) gives correct trigger matching.
- Don't capture `currentMode` inside a `DispatchWorkItem` closure as a local —
  read it at fire-time so mode switches during the silence window are honored.
- Don't take `AppleCommandRecognizer::cmd_rx` or `AppleDictationTranscriber::dict_rx`
  more than once. Each receiver has a single logical consumer.
- Don't forget to send `MODE COMMAND` back after dictation — even in the error
  path. Otherwise the helper stays in dictation mode and commands stop firing.
- Don't bump `HELPER_VERSION` without also changing `SWIFT_HELPER_SOURCE`, and
  vice versa: they must move together, otherwise users run a stale cached binary.
- Don't link AppKit/NSSound into the Swift helper — stick to Foundation,
  Speech, AudioToolbox. No AVFoundation needed anymore (no mic capture).
- Don't drop the `AecPipeline` — it holds the `cpal::Stream` that keeps the
  mic open. It's `Box::leak`ed in `backend.rs` because `cpal::Stream` is
  `!Send` and can't be stored in `BackendClient` (behind `Arc<Mutex>`).

## Code review workflow

**Before every `git push`**, spawn a code review agent using the Agent tool:

- **Review model**: `claude-opus-4-6` (change this line to use a different model)
- **Writing model**: current session model (Sonnet by default)

The review agent must:
1. Run `git diff origin/<branch>...HEAD` to see all commits being pushed
2. Check for: bugs, security issues, violations of the rules in this file,
   broken hexagonal architecture boundaries, missing error handling at system
   boundaries, regressions vs existing tests
3. Report **blocking issues** (must fix before push) and **suggestions** (optional)
4. Give a verdict: `APPROVED` or `NEEDS WORK`

If `NEEDS WORK`: present the issues, do not push, wait for the user to decide.
If `APPROVED`: proceed with push.

Use `/review-push` to trigger this manually at any time.
To override the model: `/review-push sonnet` or `/review-push haiku`.
