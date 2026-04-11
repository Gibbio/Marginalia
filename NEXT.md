# Next Steps

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
- [ ] **Resume where you left off**: persist reading position in SQLite, auto-resume on `/play`.
- [ ] **Sentence-level navigation**: skip/repeat single sentences within a chunk, not just whole chunks.

### Echo cancellation — voice commands during playback
- [x] **Text-based echo rejection** (v1, shipped): `stt-echo-filter` external crate, post-STT word-count delta filter wired in `App::handle_voice_command`. Strips TTS playback words from the STT transcript before resolving the action. Repo: https://github.com/Gibbio/stt-echo-filter
- [ ] **Platform-specific acoustic echo cancellation — evaluation matrix**: the text-based filter handles ~90% of false positives but has a known false negative (user says a word also in the chunk → dropped as echo). For the harder cases we need real acoustic AEC, and the right tool is different on each platform. Produce a comparison table covering:
    - **macOS**: AVAudioSession `voiceChat` mode (system AEC, iOS-ported to macOS?); AudioUnit `kAudioUnitSubType_VoiceProcessingIO`; WebRTC AEC3 via `webrtc-audio-processing` crate
    - **iOS**: `AVAudioSession.Category.playAndRecord` + `.voiceChat` mode (activates built-in AEC on the Neural Engine)
    - **Linux**: PipeWire `echo-cancel` module (WebRTC-based, system-wide); PulseAudio `module-echo-cancel`; WebRTC AEC3 via `webrtc-audio-processing` crate
    - **Windows**: Windows Communications APO (system-provided AEC); WASAPI loopback + WebRTC AEC3
    - **Android**: `AcousticEchoCanceler` framework class (backed by hardware DSP on most devices)
    - **Web**: `getUserMedia({audio: {echoCancellation: true}})` — browser built-in, "free"
    - **Cross-platform baseline**: `webrtc-audio-processing` / `aec3` / `aec-rs` Rust crates when the OS doesn't provide AEC
    For each row capture: availability, quality, CPU cost, integration effort, whether it requires owning the mic pipeline (which on macOS conflicts with Apple's `SFSpeechRecognizer` — see NEXT item on mic pipeline refactor).
- [ ] **Mic pipeline refactor** (prerequisite for real AEC on Apple STT): today `marginalia-stt-apple` lets the Swift helper own the mic via `AVAudioEngine.installTap`. To insert real AEC we need to either (a) run the AEC inside the Swift helper before appending buffers to `SFSpeechAudioBufferRecognitionRequest`, or (b) move mic capture into Rust (cpal), run AEC there, and feed cleaned PCM frames into the helper via a binary stdin protocol. Pick one after the evaluation matrix is done.
- [ ] Research notes: how Teams (WebRTC AEC3 + AI noise suppression), Zoom (ML isolation), Apple (Neural Engine via `voiceChat`), Alexa (DSP + multi-mic beamforming) handle this — already partly captured, finalize after the matrix above.

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
- [ ] **StyleTTS2 fine-tune** with Italian dataset (Mozilla Common Voice IT).
- [ ] **Better Italian voice embeddings** from professional speaker samples.
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
