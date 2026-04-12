# Crates

Shared Rust engine crates for Marginalia.

## Core

| Crate | Description |
|---|---|
| `marginalia-core` | Domain types, ports (traits), application services. Zero external deps. |
| `marginalia-runtime` | Orchestrates core + storage + providers into a running engine. |
| `marginalia-storage-sqlite` | SQLite persistence for documents, sessions, notes. |
| `marginalia-import-text` | Text/Markdown document importer with chunking. |

## TTS

| Crate | Backend | Platform | Performance |
|---|---|---|---|
| `marginalia-tts-mlx` | Kokoro-82M via MLX Metal GPU | macOS Apple Silicon | ~1000ms/chunk, 12x RT |
| `marginalia-tts-kokoro` | Kokoro-82M via ONNX Runtime | Cross-platform | ~5700ms/chunk, 2.3x RT |

`marginalia-tts-mlx` is auto-selected on macOS Apple Silicon. It uses
[Gibbio/voice-mlx](https://github.com/Gibbio/voice-mlx) (voice-tts 0.2 mlx-rs fork)
with [mlx-rs](https://github.com/oxideai/mlx-rs) from git for MLX C++ v0.31+.

## STT (optional, need native libs)

| Crate | Description |
|---|---|
| `marginalia-stt-apple` | Apple SFSpeechRecognizer — commands + dictation (macOS) |
| `marginalia-stt-vosk` | Vosk speech recognition (legacy, needs libvosk) |
| `marginalia-stt-whisper` | Whisper speech recognition (needs cmake + C++) |

## Other

| Crate | Description |
|---|---|
| `marginalia-playback-host` | Audio playback abstraction |
| `marginalia-provider-fake` | Fake providers for testing |
| `marginalia-devtools` | Development/diagnostic utilities |
| `marginalia-config` | Shared configuration types (voice commands, STT, TTS, playback) |
| `marginalia-models` | Model discovery, download, and cache management |
