# Marginalia

Marginalia is a local AI-first voice reading and annotation engine.

## Quick Start

Run the TUI (auto-detects platform and enables GPU acceleration):

```bash
make tui-rs
```

On **macOS Apple Silicon** this auto-enables Kokoro TTS via MLX Metal GPU (~1s latency per chunk, 12x realtime). On other platforms it falls back to ONNX Runtime CPU or fake providers.

Run tests:

```bash
cargo test
```

## Architecture

One shared Rust engine with platform-specific host applications:

```
apps/
  tui-rs/              Terminal UI (desktop dev/admin tool)
  cli-rs/              CLI for testing core (ingest, read, bench)

crates/
  marginalia-core/     Domain, ports, application services (no external deps)
  marginalia-runtime/  Orchestration: core + storage + providers
  marginalia-storage-sqlite/   SQLite persistence
  marginalia-import-text/      Text/Markdown document importer
  marginalia-tts-mlx/          Kokoro TTS via MLX Metal GPU (macOS Apple Silicon)
  marginalia-tts-kokoro/       Kokoro TTS via ONNX Runtime (cross-platform)
  marginalia-stt-vosk/         Vosk speech recognition (optional)
  marginalia-stt-whisper/      Whisper speech recognition (optional)
  marginalia-playback-host/    Audio playback
  marginalia-provider-fake/    Fake providers for testing
  marginalia-devtools/         Development utilities

models/                Local model assets (downloaded via make bootstrap-beta)
benchmark/             TTS backend benchmark suite
```

## TTS Backends

| Backend | Platform | Latency (164ch) | RTFx | Requires |
|---|---|---|---|---|
| **marginalia-tts-mlx** | macOS Apple Silicon | ~1000ms | 12x | Xcode + Metal |
| marginalia-tts-kokoro | Cross-platform | ~5700ms | 2.3x | ONNX Runtime |

On macOS Apple Silicon, `make tui-rs` automatically selects the MLX backend. The Kokoro model downloads from HuggingFace on first use (~312MB).

See [`benchmark/README.md`](benchmark/README.md) for full benchmark results across 8 backends.

## Build Targets

```bash
# Default build (no native deps required)
cargo build --release

# TUI with platform auto-detection
make tui-rs

# TUI with explicit MLX TTS (macOS Apple Silicon only)
cargo build --release -p marginalia-tui --features mlx-tts

# CLI tool
cargo run -p marginalia-cli -- help

# Optional providers (need native libs)
cargo build -p marginalia-stt-vosk      # needs libvosk
cargo build -p marginalia-stt-whisper   # needs cmake + C++ compiler
cargo build -p marginalia-tts-mlx       # needs Xcode + Metal Toolchain
```

## Bootstrap

Download model assets:

```bash
# All Beta providers
make bootstrap-beta

# Individual
make bootstrap-kokoro    # Kokoro ONNX model + voices
make bootstrap-ort       # ONNX Runtime library
make bootstrap-vosk      # Vosk STT model
make bootstrap-whisper   # Whisper STT model
```

Verify setup:

```bash
make beta-doctor
```
