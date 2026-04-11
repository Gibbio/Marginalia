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
- [ ] **Text-based echo rejection** (first step): strip chunk words from STT transcript, execute only if a trigger word remains.
- [ ] **aec3 crate**: WebRTC AEC3 for professional echo cancellation.
- [ ] Research: how Teams (WebRTC AEC3 + AI), Zoom (ML isolation), Apple (Neural Engine), Alexa (DSP multi-mic) handle this.

### Study features
- [ ] **Voice note dictation**: "nota" command activates Whisper/Apple STT in transcription mode, records until silence, saves as note attached to current position.
- [ ] **Search within document**: `/search <text>` to find and jump to a passage.
- [ ] **Notes review**: `/notes` command to list all notes and bookmarks, jump to any.
- [ ] **Export notes**: export all notes/bookmarks for a document to markdown file.

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
- [ ] **Import from Kindle highlights** (My Clippings.txt).
- [ ] **RSS/Atom feed reader** with TTS — subscribe to blogs, listen to new posts.

### Advanced audio
- [ ] **Multiple voices for dialogue**: detect speakers in text, assign different Kokoro voices.
- [ ] **Background ambient sounds** while reading (optional, for focus).
- [ ] **Spatial audio**: position the reader voice in 3D space (macOS spatial audio API).

### Community
- [ ] **Shared voice packs**: users create and share custom Kokoro voice embeddings.
- [ ] **Document library sharing**: share annotated documents with study groups.
- [ ] **Plugin system**: allow custom importers, TTS backends, STT backends via dynamic loading.
