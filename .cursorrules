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
  marginalia-stt-vosk/      Vosk speech recognition (optional, needs libvosk)
  marginalia-stt-whisper/   Whisper STT (optional, needs cmake)
  marginalia-playback-host/ Audio playback
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

Navigation commands (next/back/repeat etc.) run TTS async to avoid UI freeze.

Synthesized chunks are cached in-memory by (document_id, section, chunk, voice).
WAV files persist on disk in `.marginalia-tts-cache/`. Revisiting a chunk is instant.

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

## Key conventions

- Italian is the primary language (documents, TTS voices, STT commands)
- Chunk target: ~300 characters per chunk
- Audio: 24kHz sample rate (Kokoro), 22050Hz (Piper)
- Config: TOML (`apps/tui-rs/marginalia.toml`)
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
