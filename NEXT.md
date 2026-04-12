# Next Steps

## Beta-dev release — v0.1.0-beta (tagged)

24/25 infrastructure tasks done. Tag: `v0.1.0-beta` on beta branch.

| # | Area | Task | Status |
|---|------|------|--------|
| 1 | Architecture | Hexagonal core, all I/O via traits | Done |
| 2 | Architecture | Stable config schema (`[voice_commands]`, `[stt]`, `[stt.*]`) | Done |
| 3 | Persistence | SQLite storage for documents, sessions, notes | Done |
| 4 | Persistence | Session auto-restore on startup | Done |
| 5 | TTS | Kokoro via MLX (macOS Apple Silicon, ~1s/chunk) | Done |
| 6 | TTS | Kokoro via ONNX Runtime (cross-platform, ~5.7s/chunk) | Done |
| 7 | TTS | Cache + background prefetch (persistent across restarts) | Done |
| 8 | STT | Apple SFSpeechRecognizer, commands + dictation | Done |
| 9 | STT | Whisper, commands + dictation | Done |
| 10 | Echo | Acoustic AEC3 (WebRTC, pure Rust, cpal → aec3 → Swift helper) | Done |
| 11 | Playback | rodio in-process audio + AEC render callback | Done |
| 12 | Config | Configurable chunk size, TTS cache dir | Done |
| 13 | Logging | `log` crate across all library crates | Done |
| 14 | Shared config | `marginalia-config` crate | Done |
| 15 | RuntimeBuilder | Builder pattern, ~290 lines removed from TUI | Done |
| 16 | Events | `RuntimeEvent` enum + `RuntimeEventSink` (channels + callbacks) | Done |
| 17 | FFI | C-compatible API or UniFFI bindings | **Deferred** |
| 18 | Testing | 83 tests, full e2e flow | Done |
| 19 | CI | GitHub Actions: macOS-14 + Linux | Done |
| 20 | Docs | `cargo doc` zero warnings, all pub items documented | Done |
| 21 | Publish | Git tag `v0.1.0-beta` (crates.io deferred to stable) | Done |
| 22 | Models | `marginalia-models` crate (HuggingFace download + cache) | Done |
| 23 | STT factory | `SttEngineOutput` + `runtime.set_stt_engine()` | Done |
| 24 | espeak-rs | Compiled binding, no system espeak-ng needed | Done |
| 25 | Auto-play | Continuous reading, auto-advance on chunk end | Done |

**FFI (#17)** deferred to when the first mobile/native app starts. The
runtime is ready (RuntimeBuilder, events, JSON contract) — FFI is a thin
`extern "C"` wrapper on top.

---

## Short term — next features

### Reading flow
- [ ] **Reading speed control**: voice commands "più veloce" / "più lento" that adjust TTS speed parameter and/or rodio playback rate.
- [ ] **Sentence-level navigation**: skip/repeat single sentences within a chunk, not just whole chunks.

### Study features
- [ ] **Voice note dictation**: "nota" command activates STT in transcription mode, records until silence, saves as note attached to current position.
- [ ] **Search within document**: `/search <text>` to find and jump to a passage.
- [ ] **Notes review**: `/notes` command to list all notes and bookmarks, jump to any.
- [ ] **Export notes**: export all notes/bookmarks for a document to markdown file.

### Annotations
- [ ] **Voice highlights**: "evidenzia" command marks the current chunk; color/category for type (important / doubt / idea).
- [ ] **Tags in notes**: inline `#tag` syntax for filtering and grouping notes later.
- [ ] **Annotation timeline**: chronological view of all notes/highlights/bookmarks for a document.

### UX
- [ ] **Visual indicator during synthesis** ("synthesizing..." in status bar).
- [ ] **Progress bar** (chunk X/N, chapter X/N).
- [ ] **Volume control** via voice command and keyboard (rodio `sink.set_volume()`).
- [ ] **Reading timer**: show how long you've been reading this session.

### i18n / Localization
- [ ] All core/backend messages in English. Create translation files (`apps/tui-rs/i18n/`).
- [ ] TUI locale setting: `language = "it"` → loads Italian strings.

### TTS quality
- [ ] **TTS cloud premium**: ElevenLabs / OpenAI as optional paid backend.

### Echo cancellation — other platforms
- [ ] **iOS**: `AVAudioSession.voiceChat` (hardware AEC on Neural Engine)
- [ ] **Linux**: PipeWire `echo-cancel` module or `aec3` crate
- [ ] **Windows**: Communications APO or `aec3` crate
- [ ] **Android**: `AcousticEchoCanceler` framework class

---

## Medium term

### Import formats
- [ ] **PDF** (text extraction via pdf-extract or similar crate).
- [ ] **EPUB** (structured chapters + metadata).
- [ ] **URL import** (web scraping, reader mode extraction).
- [ ] **Markdown with images**: skip images, read alt text.

### Audio export
- [ ] **Generate audiobook**: export entire document as concatenated WAV/MP3.
- [ ] **Per-chapter export**: one audio file per chapter.

### Multi-platform
- [ ] **FFI layer**: `extern "C"` or UniFFI bindings for iOS/Android/C#.
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

---

## Long term

### Mobile
- [ ] **iOS app**: CoreML Kokoro + SFSpeechRecognizer (native Neural Engine).
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
