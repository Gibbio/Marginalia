# Next Steps

## Beta-dev release checklist

First public release aimed at developers who want to build apps on top of
Marginalia (desktop, mobile, web). Everything below must be done or explicitly
descoped before tagging `v0.1.0-beta`.

### Infrastructure — must-have

| # | Area | Task | Status | Notes |
|---|------|------|--------|-------|
| 1 | **Architecture** | Hexagonal core, all I/O via traits | Done | marginalia-core has zero platform deps |
| 2 | **Architecture** | Stable config schema (`[voice_commands]`, `[stt]`, `[stt.*]`) | Done | Final layout, documented in CLAUDE.md |
| 3 | **Persistence** | SQLite storage for documents, sessions, notes | Done | Cross-platform, portable |
| 4 | **Persistence** | Session auto-restore on startup | Done | Active session resumes in Paused state |
| 5 | **TTS** | Kokoro via MLX (macOS Apple Silicon) | Done | ~1s/chunk, 12x realtime |
| 6 | **TTS** | Kokoro via ONNX Runtime (cross-platform) | Done | ~5.7s/chunk, CPU |
| 7 | **TTS** | Cache + background prefetch | Done | Revisiting a chunk is instant |
| 8 | **STT** | Apple SFSpeechRecognizer, commands + dictation | Done | Single Swift helper, mode-switch via stdin |
| 9 | **STT** | Whisper, commands + dictation | Done | Two WhisperConfig profiles |
| 10 | **STT** | Trigger fast-path (~200ms command latency) | Done | Apple only; Whisper limited by inference |
| 11 | **Playback** | rodio in-process audio (macOS/Linux/Windows) | Done | Pause, resume, stop, sink.empty() |
| 12 | **Echo** | stt-echo-filter (text-level, post-STT) | Done | External crate, zero deps |
| 13 | **Config** | Configurable chunk size (`chunk_target_chars`) | Done | Per-platform tuning |
| 14 | **RuntimeBuilder** | Builder pattern for provider wiring | **TODO** | Eliminates 500 lines of duplicated wiring per app. Without this, every new app copy-pastes backend.rs |
| 15 | **Events / callbacks** | Runtime event system (not just polling) | **TODO** | Mobile apps need push notifications: playback finished, command recognized, synthesis ready. TUI can also benefit (replace try_recv polling) |
| 16 | **Logging** | Replace `eprintln!` with `log` crate | Done | All library crates + TUI migrated. CLI binaries keep eprintln (appropriate for CLI). |
| 17 | **FFI** | C-compatible API or UniFFI bindings | **TODO** | Required for iOS (Swift), Android (Kotlin), Windows (C#). The RuntimeFrontend JSON contract is already FFI-friendly — needs a thin `extern "C"` wrapper |
| 18 | **Testing** | Core trait tests + integration tests | **TODO** | See Testing & CI section below. Developers need to trust the library before building on it |
| 19 | **CI** | GitHub Actions: macOS (mlx, apple-stt) + Linux (onnx, whisper) | **TODO** | Compiles all crates, runs all tests on every push |
| 20 | **Docs** | `cargo doc` builds cleanly, public items documented | **TODO** | Developers read the API docs. Currently many pub items lack doc comments |
| 21 | **Crates.io** | Publish core + runtime + storage (or at minimum, stable git tags) | **TODO** | Developers need a stable dependency reference, not just `git = "..."` |
| 22 | **Shared config** | Extract `marginalia-config` crate with reusable config types | Done | VoiceCommandsSection, SttSection, KokoroSection, MlxSection, PlaybackSection extracted. TUI re-exports them. |
| 23 | **Model management** | `marginalia-models` crate: discovery, download, cache | **TODO** | Mobile apps can't run `make bootstrap-*`; need programmatic model management |
| 24 | **Unified STT factory** | Trait or function returning `(CommandRecognizer, DictationTranscriber)` from one init | **TODO** | Apple already does this via `new_apple_stt`; formalize the pattern for all engines |
| 25 | **espeak-rs** | Compiled Rust binding to eliminate system espeak-ng dependency | **TODO** | System dep is a barrier for packaging and mobile |
| 26 | **Auto-play next chunk** | Continuous reading without pressing /next | **TODO** | rodio `sink.empty()` callback; most-requested UX feature |

### Release criteria summary

**Tag `v0.1.0-beta` when rows 1–26 in the must-have table are Done.**
Features (study, annotations, AI, import formats) are post-beta — they
build on top of the infrastructure.

---

## In progress

- **Phonemizer tuning** — refine text normalization for natural Italian prosody (dialogue, parentheses, punctuation). Reference: `hexgrad/misaki/misaki/espeak.py`.

## Short term

### Testing & CI
- [ ] **Core trait tests**: verify that changes to `SpeechSynthesizer`, `CommandRecognizer`, `DocumentRepository` etc. don't break implementations.
- [ ] **Integration tests**: runtime with fake providers, full flow: ingest → start session → navigate → create note.
- [ ] **CI compiles all crates**: macOS runner for mlx/apple-stt/whisper, Linux runner for the rest.
- [ ] **Provider contract tests**: each TTS/STT implementation gets a basic smoke test.

### Reading flow
- [ ] **Auto-play next chunk** when current finishes (rodio `sink.empty()` callback). This is the most requested feature — continuous reading without pressing /next.
- [ ] **Reading speed control**: voice commands "piu' veloce" / "piu' lento" that adjust TTS speed parameter and/or rodio playback rate.
- [x] **Resume where you left off**: reading position persisted in SQLite; last active session auto-restored on TUI startup in Paused state. Type `/resume` or say "riprendi" to continue.
- [ ] **Sentence-level navigation**: skip/repeat single sentences within a chunk, not just whole chunks.

### Echo cancellation — voice commands during playback
- [x] **Acoustic AEC on macOS** (shipped): WebRTC AEC3 via `aec3` crate (pure Rust). Mic captured by cpal in Rust, processed through AEC3 (render reference = TTS WAV chunk), cleaned audio fed to Swift helper via TLV binary stdin. Helper no longer owns the mic. Trigger fast-path removed (silence timer only, fixes multi-word triggers like "prossimo capitolo").
- [ ] **Platform-specific AEC for other targets**: evaluate per-platform AEC options when building apps for those platforms:
    - **iOS**: `AVAudioSession.voiceChat` (hardware AEC on Neural Engine)
    - **Linux**: PipeWire `echo-cancel` module or `aec3` crate
    - **Windows**: Communications APO or `aec3` crate
    - **Android**: `AcousticEchoCanceler` framework class
    - **Web**: `getUserMedia({echoCancellation: true})`

### Study features
- [ ] **Voice note dictation**: "nota" command activates Whisper/Apple STT in transcription mode, records until silence, saves as note attached to current position.
- [ ] **Search within document**: `/search <text>` to find and jump to a passage.
- [ ] **Notes review**: `/notes` command to list all notes and bookmarks, jump to any.
- [ ] **Export notes**: export all notes/bookmarks for a document to markdown file.

### Annotations
- [ ] **Voice highlights**: "evidenzia" command marks the current chunk; color/category for type (important / doubt / idea).
- [ ] **Tags in notes**: inline `#tag` syntax for filtering and grouping notes later.
- [ ] **Annotation timeline**: chronological view of all notes/highlights/bookmarks for a document.

### UX
- [ ] **Visual indicator during synthesis** ("synthesizing..." in status bar).
- [ ] **Progress bar** (chunk X/N, chapter X/N) in the TUI header.
- [ ] **Volume control** via voice command and keyboard (rodio `sink.set_volume()`).
- [ ] **Reading timer**: show how long you've been reading this session.

### i18n / Localization
- [ ] All core/backend messages in English. Create translation files (`apps/tui-rs/i18n/`).
- [ ] TUI locale setting: `language = "it"` → loads Italian strings.
- [ ] Voice commands already configurable in toml — no code changes for new languages.

### TTS quality
- [ ] **espeak-rs**: compiled Rust binding to eliminate system espeak-ng dependency.
- [ ] **TTS cloud premium**: ElevenLabs / OpenAI as optional paid backend.

## Medium term

### Import formats
- [ ] **PDF** (text extraction via pdf-extract or similar crate).
- [ ] **EPUB** (structured chapters + metadata).
- [ ] **URL import** (web scraping, reader mode extraction).
- [ ] **Markdown with images**: skip images, read alt text.

### Audio export
- [ ] **Generate audiobook**: export entire document as concatenated WAV/MP3.
- [ ] **Per-chapter export**: one audio file per chapter.
- [ ] Useful for offline listening (commute, gym) without the app running.

### Multi-platform
- [ ] **Linux**: test Kokoro ONNX on CPU, Whisper STT.
- [ ] **Tauri desktop app**: wraps a web UI, ships as native .app / .deb / .exe.
- [ ] **Windows**: DirectML or CUDA for TTS.

### AI features
- [ ] **Summarize chapter**: LLM generates a summary of the current chapter on demand.
- [ ] **Explain passage**: select a passage, ask the LLM to explain it.
- [ ] **Quiz generation**: generate questions from what you've read (study mode).
- [ ] **Translation on the fly**: read a document in one language, get translation in another.

### Dependencies
- [ ] Monitor `mlx-rs` for crates.io updates (MLX C++ v0.31+).
- [ ] Monitor `voice-tts` / `voice-nn` for upstream mlx-rs return.
- [ ] Evaluate `compile_with_state` for JIT decoder when mlx-rs improves.

## Long term

### Mobile
- [ ] **iOS app**: CoreML Kokoro + SFSpeechRecognizer (native Neural Engine for both TTS and STT).
- [ ] **Android app**: ONNX Runtime + Android SpeechRecognizer API.
- [ ] **Reading position sync** across devices.

### Integrations
- [ ] **Export notes to Obsidian/Notion/Markdown** (via file or API).
- [ ] **RSS/Atom feed reader** with TTS — subscribe to blogs, listen to new posts.

### Advanced audio
- [ ] **Background ambient sounds** while reading (optional, for focus).
- [ ] **Spatial audio**: position the reader voice in 3D space (macOS spatial audio API).

### Community
- [ ] **Shared voice packs**: users create and share custom Kokoro voice embeddings.
- [ ] **Document library sharing**: share annotated documents with study groups.
- [ ] **Plugin system**: allow custom importers, TTS backends, STT backends via dynamic loading.
