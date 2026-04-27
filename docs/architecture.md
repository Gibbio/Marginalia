# Marginalia — architecture

Marginalia is a Rust workspace organized around a small, dependency-free
domain core surrounded by trait-shaped ports. Concrete adapters
(SQLite, MLX TTS, Apple SFSpeechRecognizer, WebRTC AEC3, …) implement
those ports against real backends; a runtime crate composes them; host
apps (TUI, CLI, mac-gui) drive the runtime and render the user
interface.

This document is the bird's-eye view: what each crate owns, where
the boundaries are, how the pieces talk to each other, and the parts
that are unusual enough to call out (the TTS cache, the STT echo-
cancellation pipeline, the FFI contract).

If you're about to change code, this doc plus the relevant crate's
own `README.md` should be enough to plan the change without grepping
through the whole repo.

---

## Birds-eye

```
  ┌─────────────────────────────────────────────────────────────┐
  │                          host apps                          │
  │   apps/tui-rs   apps/cli-rs   apps/mac-gui (SwiftUI + FFI)  │
  └──────────────────────────────┬──────────────────────────────┘
                                 │ JSON commands / events
  ┌──────────────────────────────┴──────────────────────────────┐
  │                    marginalia-runtime                       │
  │       (composition, async navigation, prefetch, cache)      │
  └──────────────────────────────┬──────────────────────────────┘
                                 │ trait calls
  ┌──────────────────────────────┴──────────────────────────────┐
  │                   marginalia-core::ports                    │
  │   SpeechSynthesizer  CommandRecognizer  DictationTranscriber│
  │   DocumentRepository  SessionRepository  NoteRepository     │
  │   DocumentImporter    PlaybackEngine                        │
  └─┬────────┬──────────┬──────────┬──────────┬──────────┬─────┘
    │        │          │          │          │          │
   tts-mlx  tts-       stt-      stt-       playback   storage-
   tts-     kokoro     apple     whisper    -host      sqlite
   (MLX)    (ONNX)     (Swift)   (ggml)     (rodio)    (sqlite)
   ┌─ aec3 (WebRTC AEC3 port — pure Rust) wired into stt-apple ─┐
   ┌─ import-{text,pdf,epub,url}      → DocumentImporter port  ─┐
```

The arrows go top-to-bottom: apps depend on runtime, runtime depends
on core ports, adapters depend on core ports (which they implement).
Core has zero outgoing dependencies on the rest of the workspace.

---

## Hexagonal in one paragraph

The core (`marginalia-core`) is a pure-logic crate: domain types
(`Document`, `Session`, `VoiceNote`, `SynthesisRequest`, …),
application services that orchestrate behavior in terms of trait
methods, and the trait definitions themselves (the "ports"). It has
**zero I/O**, no `tokio`, no filesystem, no audio, no SQL — just
data and logic. That's what makes it testable in isolation and
portable across hosts.

Adapters live in their own crates and implement the ports. Each
adapter brings its own native dependencies (Metal for MLX,
SFSpeechRecognizer for Apple STT, etc.) and its own version of "the
right way" to do that thing. The core never knows or cares.

The runtime crate composes everything: it owns the live `Runtime`
object, holds boxed trait implementations, threads through events,
manages the TTS cache, and exposes a JSON-shaped frontend API the
host apps drive.

Why this matters in practice:
- Adding a new TTS engine = one new crate that implements
  `SpeechSynthesizer`. No core changes.
- Swapping SQLite for something else = one crate that implements
  the three storage repositories. No application-service changes.
- Testing chunk-advance logic = unit tests in core, with fake
  trait impls provided by `marginalia-provider-fake`. No native
  deps.

---

## Crate map

| Crate | Role | Key types / traits |
|---|---|---|
| `marginalia-core` | Domain types + port traits + application services. Zero deps on the rest of the workspace. | `Document`, `Section`, `Chunk`, `Session`, `VoiceNote`, `SynthesisRequest`, `SpeechSynthesizer`, `DocumentRepository`, `SessionRepository`, `NoteRepository`, `DocumentImporter`, `PlaybackEngine`, `CommandRecognizer`, `DictationTranscriber` |
| `marginalia-config` | Stable TOML schema. Shared by every crate that reads `marginalia.toml`. | `AppConfig`, `VoiceCommandsSection`, `SttSection`, `MlxSection`, `KokoroSection`, `PlaybackSection` |
| `marginalia-runtime` | Composition + per-session state. Owns the boxed providers, runs prefetch, drives the JSON frontend API, emits typed events. | `Runtime`, `RuntimeBuilder`, `RuntimeFrontend`, `RuntimeEvent`, `SidecarInit` |
| `marginalia-storage-sqlite` | SQLite implementation of the three repositories. | `SQLiteDocumentRepository`, `SQLiteSessionRepository`, `SQLiteNoteRepository` |
| `marginalia-tts-mlx` | Kokoro 82M via Apple MLX (Metal). macOS Apple Silicon only. | `MlxKokoroSynthesizer` |
| `marginalia-tts-kokoro` | Kokoro 82M via ONNX Runtime. Cross-platform fallback. | `KokoroSynthesizer` |
| `marginalia-stt-apple` | Apple `SFSpeechRecognizer` via a Swift helper subprocess + AEC3 echo-cancellation pipeline. | `AppleCommandRecognizer`, `AppleDictationTranscriber`, `AecPipeline` |
| `marginalia-stt-whisper` | Whisper.cpp via `whisper-rs`. Two `WhisperConfig` profiles (commands vs dictation). | `WhisperCommandRecognizer`, `WhisperDictationTranscriber` |
| `marginalia-stt-vosk` | Vosk STT (legacy, no longer wired in TUI). Kept compiling for posterity. | — |
| `marginalia-playback-host` | rodio sink + AEC render callback. Owns the audio output thread. | `HostPlaybackEngine`, `AecRenderSlot` |
| `marginalia-import-text` | `.txt` / `.md` importer with sentence-aware chunking. | `TextImporter` |
| `marginalia-import-pdf` | PDF text extraction via PDFium. | `PdfImporter` |
| `marginalia-import-epub` | EPUB 2/3 importer (pure Rust, `epub` + `scraper`). | `EpubImporter` |
| `marginalia-import-url` | Web article importer (ureq + readability-rust). | `UrlImporter` |
| `marginalia-models` | HuggingFace download / cache management. | `ModelManager` |
| `marginalia-provider-fake` | In-memory fake providers used by core tests and dev scenarios. | `InMemoryDocumentRepository`, `FakeSpeechSynthesizer`, … |
| `marginalia-ffi` | UniFFI bindings consumed by the mac-gui. | `FfiRuntime`, `WaveformSnapshot`, `ClearNotesReport` |
| `marginalia-devtools` | Dev utilities: bench harness, smoke runners. | — |

Apps:

| App | What it is |
|---|---|
| `apps/tui-rs` | Ratatui-based terminal UI. The original development surface; still useful for headless work and quick smoke testing. |
| `apps/cli-rs` | Plain CLI for ingesting documents, running benches, dumping debug info. No interactive UI. |
| `apps/mac-gui` | SwiftUI macOS app. Talks to the runtime through `marginalia-ffi`. The primary user-facing target. |

---

## Port traits

Defined in `marginalia-core::ports`. Each port is a Rust trait with
a single conceptual responsibility.

| Port | Implementations | What the runtime asks it for |
|---|---|---|
| `SpeechSynthesizer` | `marginalia-tts-mlx`, `marginalia-tts-kokoro`, `marginalia-provider-fake` | Render text to PCM/WAV/FLAC for a given voice + language |
| `CommandRecognizer` | `marginalia-stt-apple`, `marginalia-stt-whisper`, `marginalia-provider-fake` | Listen for short utterances, return the recognized text |
| `DictationTranscriber` | `marginalia-stt-apple`, `marginalia-stt-whisper`, `marginalia-provider-fake` | Listen for long utterances (notes), return the finalized transcript |
| `DocumentImporter` | `marginalia-import-{text,pdf,epub,url}`, `DispatchImporter` (in runtime) | Parse a source into a `Document` with sections + chunks |
| `DocumentRepository` | `marginalia-storage-sqlite`, `marginalia-provider-fake` | Persist + query documents |
| `SessionRepository` | `marginalia-storage-sqlite`, `marginalia-provider-fake` | Persist + query reading sessions (current position, history) |
| `NoteRepository` | `marginalia-storage-sqlite`, `marginalia-provider-fake` | Persist + query voice notes / bookmarks |
| `PlaybackEngine` | `marginalia-playback-host` | Play a synthesized audio file, emit `PlaybackFinished` when done |

The runtime is generic over these traits — it owns
`Box<dyn SpeechSynthesizer + Send>`, etc. Apps construct a runtime
by calling setters: `set_speech_synthesizer(box)`, `set_command_recognizer(box)`,
and so on. The `RuntimeBuilder` is sugar over those setters that
also wires the storage layer and (on the apple-stt + host-playback
build) the AEC pipeline.

---

## TTS critical path

TTS latency is the dominant UX constraint. The runtime applies three
mechanisms to keep "first chunk" near 1 s and subsequent chunks
instant.

**1. Content-addressed cache.** The cache key is
`sha256(text + voice + language)`. Files on disk are named by the
hex digest. Implications:

- Identical chunks across documents share a cache entry (you can
  re-import the same PDF and only the new chunks resynthesize).
- Editing a chunk's text changes the key → automatic cache miss →
  fresh synthesis. No stale audio.
- Deleting a document leaves orphaned cache entries (~50 KB each).
  Acceptable; the user-facing "clear cache" wipes the directory
  wholesale.

The cache lives on disk under `<config_dir>/.marginalia/tts-cache/`
by default. The runtime also keeps an in-memory map for the hot path
(see step 1 of `synthesize_cached` in `marginalia-runtime/src/lib.rs`).

**2. Async navigation.** Voice / keyboard commands (`next`, `back`,
`repeat`, `next_chapter`, …) run in a background thread. The UI never
blocks on synthesis. Apps poll the runtime for events and update
when `ChunkAdvanced` / `PlaybackFinished` arrives.

**3. Background prefetch.** After a navigation command completes,
a **separate** fire-and-forget thread pre-synthesizes the next
chunk into the cache. The prefetch thread acquires the runtime lock
only after the command thread releases it, so the UI stays
responsive. The thread is spawned from `poll_async_result` (after
the command succeeds) — never from `replay_session_at_position`.

The combined effect: first chunk in a session ~1 s on Apple Silicon,
every chunk after that effectively zero (cache hit on the prefetched
file).

---

## STT pipeline (Apple) — AEC3 + Swift helper

This is the most architecturally unusual subsystem and the one
where most contributors get lost without the diagram. The pipeline
has three layers.

```
WAV chunk ──callback──→ AEC3 (render reference)
                              │
Mic (cpal) ──resample 24k──→ AEC3 (capture) ──cleaned──→ Swift helper (stdin TLV)
                                                              │
                                                     SFSpeechRecognizer
                                                              │
                                                     CMD/DICT_END (stdout)
                                                              │
                                                     Rust reader thread
```

**Layer 1 — Acoustic echo cancellation.** Rust captures the mic
through `cpal`, resamples to 24 kHz mono, and feeds each 10 ms
frame through `aec3::voip::VoipAec3` (a pure-Rust port of WebRTC
AEC3). The render reference (the audio currently being played by
the TTS) comes through a callback on `HostPlaybackEngine`: when a
chunk starts playing, the WAV samples are sent to the AEC thread
which advances frame-by-frame in lockstep with mic capture. AEC3
subtracts the reference from the mic; the Swift helper never sees
the TTS echo.

This matters: without AEC3, a chunk of text containing the word
"pausa" would pause the player when it played. With AEC3, the
recognizer sees the cleaned signal and stays quiet during playback.

**Layer 2 — Swift helper subprocess.** The helper does NOT own the
mic. It reads pre-cleaned audio from stdin via a binary TLV protocol:

- Type `0x41` (`'A'`): audio frame (f32 samples, little-endian, 10 ms @ 24 kHz)
- Type `0x4D` (`'M'`): mode command (`COMMAND` or `DICTATION`)

The helper wraps audio frames in `AVAudioPCMBuffer` and feeds them
to `SFSpeechAudioBufferRecognitionRequest`. Recognized text goes to
stdout with prefixes:

- `CMD <text>` — command-mode utterance (short, hands-free triggers)
- `DICT_END <text>` — finalized dictation (long, voice-note text)

Mode switching triggers `scheduleRestart()` which ends the current
recognition task and starts a fresh one with the new mode's silence
timer.

The helper's source is embedded as a string constant
(`SWIFT_HELPER_SOURCE`) in `marginalia-stt-apple/src/lib.rs` and
compiled to `$TMPDIR/marginalia-stt-apple/stt-helper-vN` on first
run. `HELPER_VERSION` and `SWIFT_HELPER_SOURCE` move together —
bumping one without the other means users silently run a stale
binary.

**Layer 3 — Rust consumers.** `AppleCommandRecognizer` (commands)
and `AppleDictationTranscriber` (notes) share the helper process
through `Arc<AppleHelperShared>`. A single reader thread parses
stdout lines and routes them to two `mpsc` channels based on the
prefix.

The constructor returns all three together — `(recognizer,
dict_transcriber, aec_pipeline)`. The runtime keeps the
`AecPipeline` alive (in `RuntimeSidecar.aec_pipeline`); dropping
it stops the cpal mic stream cleanly, which is what lets us
respawn the helper on language change without restarting the app.

---

## Mac GUI ↔ runtime (FFI)

The mac-gui is a SwiftUI app that talks to the Rust runtime through
UniFFI-generated bindings.

```
SwiftUI views        @ObservedObject FFIHost (Swift)
        │                       │
        │   commands             │   typed events
        ▼                       ▼
   FfiRuntime methods       runtime.poll_events()
        │                       ▲
        ▼                       │
              marginalia-ffi (Rust, UDL-defined)
        │                       │
        ▼                       │
              marginalia-runtime::Runtime (Rust)
```

Build flow:

1. `make build-xcframework` compiles `marginalia-ffi` as a cdylib
   for arm64-apple-darwin, runs `uniffi-bindgen` against the dylib's
   metadata, packages the result as `MarginaliaKit.xcframework` and
   drops `Marginalia.swift` into `apps/mac-gui/Generated/`.
2. The SwiftPM package at `apps/mac-gui/MarginaliaUI/` imports
   `MarginaliaKit` and uses `FfiRuntime` like a native Swift class.
3. `make bundle-live` builds the SwiftUI app, embeds the xcframework
   inside the `.app` bundle, code-signs, and lays out helpers.

Key files:

- `crates/marginalia-ffi/src/marginalia.udl` — the UDL surface
  (commands, queries, events, dictionary types like
  `WaveformSnapshot`, `ClearNotesReport`)
- `crates/marginalia-ffi/src/lib.rs` — `FfiRuntime` implementation;
  wraps `marginalia-runtime::Runtime` behind a `Mutex`, manages
  the sidecar thread
- `apps/mac-gui/Generated/Marginalia.swift` — generated, don't edit;
  rebuild via `make build-xcframework` after every UDL or `lib.rs`
  change
- `apps/mac-gui/MarginaliaUI/Sources/MarginaliaUI/RuntimeBridge.swift` —
  the `MarginaliaHost` adapter that bridges `FfiRuntime` into
  SwiftUI's `@Published` model

---

## Storage

SQLite (`marginalia-storage-sqlite`). One file per install, default
location `<config_dir>/.marginalia/beta.sqlite3`.

Tables (high-level):

| Table | What's in it |
|---|---|
| `documents` | document metadata (id, title, source path, ingest checksum) |
| `sections` | per-document chapters / spine items, ordered |
| `chunks` | text + section_index + chunk_index + character offsets |
| `sessions` | reading position (document, section, chunk), one active session at a time |
| `notes` | voice / typed notes, anchored to (document, section, chunk), optional `raw_audio_path` |

Schema is intentionally simple — no migrations infrastructure today,
the app is in beta with one user. The export/import backup feature
(`/export_backup`, `/import_backup`) writes a zip with the TOML +
the sqlite db + the voices manifest, plaintext.

---

## Configuration

The TOML schema lives in `marginalia-config` and is intentionally
stable. The runtime, FFI, TUI, and mac-gui all read the same shape.

```toml
[voice_commands]                         # trigger words → actions
pause = ["pausa", "ferma"]
next = ["avanti", "prossimo"]
# ...

[stt]                                    # engine selection + shared options
engine   = "apple"                       # "apple" or "whisper"
language = "it"                          # ISO ("it") or BCP-47 ("it-IT")
debug    = true

[stt.apple]                              # apple-engine settings
[stt.whisper]                            # whisper-engine settings
model_path = "models/stt/whisper/ggml-small.bin"

[stt.commands]                           # short-utterance tuning
silence_timeout    = 0.8
max_record_seconds = 4
speech_threshold   = 500

[stt.dictation]                          # long-utterance tuning
silence_timeout    = 1.5
max_record_seconds = 60
speech_threshold   = 500

[mlx]
model = "prince-canuma/Kokoro-82M"        # HF repo or local path
voice = "if_sara"                         # default voice id
```

Resolution order:

1. `apps/tui-rs/marginalia.toml` for the TUI (template-generated by
   `make tui-rs` on first run).
2. `~/Library/Application Support/Marginalia/marginalia.toml` for
   the mac-gui.
3. CLI may take a `--config` path explicitly.

The app reads, the FFI exposes `config_path()`, and the mac-gui's
Settings page round-trips most of the schema through `save_config()`.

---

## Audio playback

`marginalia-playback-host` owns the rodio output sink, exposes a
`HostPlaybackEngine` that implements the `PlaybackEngine` port,
and provides the AEC render-reference hook.

The mac-gui takes a hybrid approach:

- **TTS chunk playback** goes through `HostPlaybackEngine` (rodio)
  so the AEC render callback fires automatically and the helper
  sees a cleaned mic.
- **Voice-note playback** uses `AVAudioPlayer` directly in Swift
  (faster Swift-side seek, native scrubbing). The AEC render slot
  is fed from Swift via an explicit `aecSetRenderReference(path:)`
  call so notes whose body contains the word "pausa" don't
  auto-fire commands when the user replays them.

---

## Testing

The hexagonal split makes testability cheap.

- **Core unit tests**: in `marginalia-core`. Pure-logic, no native
  deps. Use `marginalia-provider-fake` for trait stubs. ~70 tests.
- **Provider smoke tests**: each provider crate has its own tests
  guarded behind `#[cfg(test)]` and feature flags. They run
  conditionally — the CI matrix splits Linux (default features)
  from macOS (apple-stt + mlx-tts).
- **End-to-end**: a small set of e2e tests in `marginalia-runtime`
  drives the JSON frontend API against the in-memory fake provider
  graph. They verify the public command/event contract — the same
  one the TUI and mac-gui depend on.

`cargo test` from the workspace root runs the portable subset.
Native-dep tests run on the macOS CI runner only.

---

## Where to look when…

| Question | Start here |
|---|---|
| What does `next_chunk` actually do? | `marginalia-runtime/src/lib.rs::Runtime::advance` |
| How is the cache filename derived? | `marginalia-runtime/src/lib.rs::synthesize_cached` |
| Why does the Swift helper exist? | `crates/marginalia-stt-apple/src/lib.rs` (`SWIFT_HELPER_SOURCE`) |
| How does a new TTS backend slot in? | `marginalia-core::ports::SpeechSynthesizer` + an existing impl in `marginalia-tts-mlx` for shape |
| How does the mac-gui get the runtime? | `apps/mac-gui/MarginaliaUI/Sources/MarginaliaUI/RuntimeBridge.swift::FFIHost` |
| What's the FFI surface? | `crates/marginalia-ffi/src/marginalia.udl` |
| Where does config go on disk? | `apps/mac-gui/Marginalia/MarginaliaApp.swift::resolveConfigPath` |
| How is voice-command resolution done? | `marginalia-config/src/lib.rs::VoiceCommandsSection::resolve_action` |
