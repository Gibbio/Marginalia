# Next Steps

## In progress

- **Phonemizer tuning** — refine text normalization for natural Italian prosody (dialogue, parentheses, punctuation). Reference: `hexgrad/misaki/misaki/espeak.py`.

## Short term

### i18n / Localization
- [ ] All core/backend messages must be in **English**. The TUI currently mixes Italian and English — standardize to English.
- [ ] Create a separate translation file for TUI user-facing strings (`apps/tui-rs/i18n/` or similar). This includes:
  - Log pane messages ("Bookmark saved", "No active session", "Position: ...")
  - Status messages ("Starting playback...", "Busy — please wait...")
  - Command descriptions ("/play", "/pause", etc.)
- [ ] Voice commands are already configurable in the toml (`[voice_commands]`) — users set trigger words in their language. No code changes needed for new languages.
- [ ] TUI locale setting in config: `language = "it"` → loads Italian strings.

### Italian TTS quality
- [ ] Evaluate StyleTTS2 fine-tune with Italian dataset (Mozilla Common Voice IT, ~100h free). Training: GPU 24GB+, 2-3 days. Produces ONNX drop-in replacement for Kokoro.
- [ ] Create better Italian voice embeddings from professional speaker samples.
- [ ] Explore `espeak-rs` (compiled Rust binding) to eliminate system `espeak-ng` dependency.

### TTS cloud premium
- [ ] Integrate ElevenLabs and/or OpenAI TTS as optional paid backend. REST API, implement `SpeechSynthesizer` with an HTTP crate. User chooses local (free, ~1s) or cloud (paid, ~100ms, higher quality especially for Italian).
- [ ] Config:
  ```toml
  [tts]
  provider = "mlx"  # or "elevenlabs", "openai"

  [elevenlabs]
  api_key = "..."
  voice_id = "..."
  ```

### UX
- [ ] Auto-play next chunk when current finishes (continuous reading without pressing /next).
- [ ] Visual indicator during synthesis ("synthesizing...").
- [ ] Progress bar (chunk X/N).
- [ ] Voice note dictation: "nota" command activates Whisper transcriber, records until silence, saves as note.

## Medium term

### Multi-platform
- [ ] Test and optimize Kokoro ONNX on Linux (CPU). May need XNNPACK or different backend for ARM Linux.
- [ ] Evaluate TTS for Windows (DirectML, CUDA).
- [ ] Desktop app with Tauri (wraps TUI or a web UI).

### Dependencies
- [ ] Monitor `mlx-rs` for new crates.io releases — when it includes MLX C++ v0.31+, remove git dependency and use stable version.
- [ ] Monitor `voice-tts` / `voice-nn` for updates — if the author returns to mlx-rs, align with their repo instead of maintaining `Gibbio/voice-mlx` fork.
- [ ] Evaluate `compile_with_state` for JIT decoder compilation when mlx-rs supports it better. Potential -30% latency.

### Import
- [ ] PDF support (text extraction).
- [ ] EPUB support.
- [ ] Import from URL (web scraping).

## Long term

### Mobile
- [ ] iOS app with native CoreML Kokoro (FluidInference/kokoro-82m-coreml model, benchmarked 23x RTF on M4).
- [ ] Android app with ONNX Runtime (CPU or NNAPI).

### STT
- [ ] Voice note dictation via Whisper transcriber (record → transcribe → save as note).
- [ ] Evaluate larger Whisper models (medium, large-v3-turbo) for better accuracy.
- [ ] **Apple native STT** (`SFSpeechRecognizer`) via `objc2-speech` crate (v0.3.2). Runs on Neural Engine — faster and more accurate than Whisper, zero models to download. New crate `marginalia-stt-apple` implementing `CommandRecognizer`. macOS/iOS only (cfg target). Could replace Whisper as default on Apple platforms.
- [ ] Android native STT (`SpeechRecognizer` API) for Android app.

### Sync
- [ ] Reading position sync across devices.
- [ ] Cloud backup for documents and notes (optional).
