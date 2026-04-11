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

### Apple STT — single Swift helper, dual mode

This is the crucial bit to get right when extending Apple STT (and the model
to copy when we add other hosts). **ONE** Swift helper process (`stt-helper`,
spawned by `marginalia-stt-apple`) handles both command recognition and note
dictation. Mode is switched at runtime via stdin control lines:

- Rust writes `MODE COMMAND\n` / `MODE DICTATION\n` to the helper's stdin
- Swift helper reads on a background queue, dispatches the mode change to
  main queue, calls `scheduleRestart()` so the new mode takes effect on the
  next recognition task
- Output lines are PREFIXED so the Rust reader can route them:
  - `CMD <text>` → command channel (cmd_rx)
  - `DICT_END <text>` → dictation channel (dict_rx)
- A single `std::thread` on the Rust side reads stdout and dispatches lines
  to the right mpsc channel based on prefix

In **command mode**: short silence timer, fast-path emits immediately when a
partial already contains a trigger word ("avanti", "stop", …).

In **dictation mode**: longer silence timer, NO trigger fast-path (user might
legitimately say "stop" in a note), accumulates text until silence, emits the
finalized utterance as `DICT_END <full text>`.

Both consumers share the same child process through `Arc<AppleHelperShared>`:
`stdin: Mutex<ChildStdin>` for serialized mode commands, `child: Mutex<Child>`
kept alive until the last Arc drops (at which point Drop kills the process).

Constructor returns BOTH sides together — do not build them separately:
```rust
let (recognizer, dict_transcriber) = new_apple_stt(
    language, commands, cmd_silence, dict_silence, dict_max
)?;
runtime.set_command_recognizer(recognizer);
runtime.set_dictation_transcriber(dict_transcriber);
```

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

### Echo filter — the TTS talking to itself

When the TTS reads a document aloud and the mic picks up the playback, the
STT transcribes the TTS audio as if the user had spoken. Trigger words inside
the document would then fire spurious commands.

To prevent this, `apps/tui-rs` depends on **`stt-echo-filter`** (external
crate, https://github.com/Gibbio/stt-echo-filter), a tiny pure-Rust library
that strips playback echo from STT transcripts at the WORD level. It's a
**post-STT** filter, not an acoustic one — it doesn't touch audio, only text.

Algorithm: per-word budget. Each word in the currently-playing chunk
"consumes" one occurrence in the STT output. Surplus words form a delta that
represents what the user actually said, in order. Multi-word triggers like
`"prossimo capitolo"` still work because the delta preserves order.

Where it's wired: `App::handle_voice_command(raw: &str)` takes the raw STT
utterance, looks up `session_snapshot.chunk_text` and `playback_state`, and
— only when `playback_state == "playing"` — calls `stt_echo_filter::strip_echo`
before passing the result to `voice_commands.resolve_action`. If the filter
absorbs everything, a debug line is logged to the Log pane and no action is
fired. The monitor thread still pre-matches a command internally but that
pre-match is intentionally **ignored** by `main.rs`; the filtering happens
later so it can see the playback state.

Known trade-off: if the user legitimately says a word that is ALSO in the
current chunk text, the budget absorbs it as echo and the command is
dropped. Mitigation: use synonyms in `[voice_commands]`, or speak between
chunks. For harder cases we may one day add real acoustic AEC — see
"Advanced echo handling" in NEXT.md.

**Do not** try to apply the filter inside the STT monitor thread — it
doesn't know about playback state. It lives in `App` where session state is
available. And **do not** remove the `_cmd` channel field in
`poll_voice_event` even though main.rs ignores it — other future consumers
(e.g. a pre-playback command prompt) may still want the pre-matched hint.

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
  shared process with mode-switch-via-stdin is the deliberate architecture
  (no mic contention, no duplicate recognition, no doubled memory). Extend
  `SWIFT_HELPER_SOURCE` instead of spawning another helper.
- Don't capture `currentMode` inside a `DispatchWorkItem` closure as a local —
  read it at fire-time so mode switches during the silence window are honored.
- Don't take `AppleCommandRecognizer::cmd_rx` or `AppleDictationTranscriber::dict_rx`
  more than once. Each receiver has a single logical consumer.
- Don't forget to send `MODE COMMAND` back after dictation — even in the error
  path. Otherwise the helper stays in dictation mode and commands stop firing.
- Don't bump `HELPER_VERSION` without also changing `SWIFT_HELPER_SOURCE`, and
  vice versa: they must move together, otherwise users run a stale cached binary.
- Don't link AppKit/NSSound into the Swift helper — stick to Foundation,
  Speech, AVFoundation, AudioToolbox. No GUI runtime dependency.
- Don't apply the `stt-echo-filter` inside the STT crates or the monitor
  thread — it lives in `App::handle_voice_command` where the session
  snapshot (chunk text + playback state) is reachable. The monitor thread
  is deliberately dumb about playback.
- Don't vendor `stt-echo-filter` into the Marginalia tree — it is an
  independent crate at https://github.com/Gibbio/stt-echo-filter, pulled
  via git dependency in `apps/tui-rs/Cargo.toml`. Changes to the algorithm
  go there, then we bump the git reference here.
