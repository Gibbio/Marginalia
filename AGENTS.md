# AGENTS.md — Marginalia

This repository is part of the **Marginalia** project.

Marginalia is a local-first voice reading and annotation engine
written in Rust. It ingests text/markdown documents, chunks them,
synthesizes speech via TTS, and lets the user navigate, annotate, and
interact with the content by voice.

The project is **not** an audiobook player, **not** a generic
dictation app, **not** a TTS toy. It runs offline; the network is
touched only on explicit asset download.

## Required reading before editing

Before modifying anything in this repository, read:

- `README.md`
- `CONTRIBUTING.md` if present
- this `AGENTS.md`
- the relevant crate / app README under `crates/<x>/README.md` or
  `apps/<x>/README.md`

If the maintainer has provided additional project context for the
session, follow it. If unsure, stop and ask the maintainer before
editing — do not proceed on partial context.

## This repository's role

This repository ships the **engine and apps**:

- `apps/tui-rs` — Terminal UI, the main desktop interface
- `apps/cli-rs` — CLI for testing core (ingest, read, bench)
- `apps/mac-gui` — SwiftUI Mac app, FFI to the Rust runtime
- `crates/marginalia-core` — domain types + ports (no I/O)
- `crates/marginalia-runtime` — composes core + storage + providers
- `crates/marginalia-storage-sqlite` — SQLite persistence
- `crates/marginalia-import-{text,pdf,epub,url}` — document importers
- `crates/marginalia-tts-{mlx,kokoro}` — TTS backends
- `crates/marginalia-stt-{apple,whisper,vosk}` — STT backends
- `crates/marginalia-playback-host` — audio playback (rodio)
- `crates/marginalia-provider-fake` — fake providers for testing
- `crates/marginalia-ffi` — UniFFI bindings consumed by the mac-gui
- `crates/marginalia-config` — TOML schema
- `crates/marginalia-devtools` — dev utilities

It must **not** contain:

- a desktop / web UI other than tui-rs and mac-gui
- a server or remote service
- payment / accounts / multi-user infrastructure

## Build

```bash
cargo build --release                                          # default
cargo build --release -p marginalia-tui --features mlx-tts     # with MLX TTS
make tui-rs                                                    # auto-detects platform
make build-xcframework                                         # mac-gui FFI bindings
make bundle-live                                               # mac-gui .app bundle
cargo test                                                     # all tests
```

Building `marginalia-tts-mlx` requires Xcode + Metal Toolchain on
macOS Apple Silicon.

## Architecture (one paragraph)

Hexagonal. Core has zero external deps; all I/O through traits in
`marginalia-core::ports`:

- `SpeechSynthesizer` — TTS
- `DocumentRepository`, `SessionRepository`, `NoteRepository` — storage
- `DocumentImporter` — file import
- `PlaybackEngine` — audio playback
- `CommandRecognizer`, `DictationTranscriber` — STT

The runtime (`marginalia-runtime`) composes everything. Apps create
providers and pass them to the runtime via `set_speech_synthesizer()`,
`set_command_recognizer()`, etc. The mac-gui talks to the runtime
through the FFI in `crates/marginalia-ffi`.

## Mandatory project-wide rule

Do **not** make isolated changes.

Every change must consider:

- TUI behavior (commands, status bar)
- CLI behavior (flags, output, exit codes)
- mac-gui behavior (after FFI rebuild)
- port traits (do all adapters still compile?)
- `marginalia.toml` schema
- TTS critical path (latency, cache key, prefetch)
- STT impact (Apple helper, Whisper, AEC3)
- README/docs
- tests

A change is incomplete if it changes behavior without updating the
relevant docs/spec/tests in the same patch.

## Impact analysis

Before non-trivial changes, walk these:

1. summarize the change in one sentence
2. list affected crates / apps
3. does it touch a port trait? a provider? config schema? FFI surface?
4. tests or docs to update
5. only then write code

## Conventions

- Italian is the primary language (documents, voices, voice commands);
  English is supported throughout
- Chunk target: ~300 characters per chunk
- Audio: 24 kHz (Kokoro), 22050 Hz (Piper)
- Config: TOML at `apps/tui-rs/marginalia.toml`, generated from
  template by `make tui-rs`. The mac-gui resolves its config at
  `~/Library/Application Support/Marginalia/marginalia.toml`
- Default voice: `af_bella` (English), `if_sara` / `im_nicola`
  (Italian)
- espeak-ng is used as external phonemizer (all languages,
  clause-by-clause)
- The mic stream stays open for the entire session — never open/close
  per capture cycle

## Definition of done

A change is not complete unless:

- code builds (`cargo build --release` for engine, `swift build` for
  the mac-gui)
- tests pass or are updated (`cargo test`)
- docs are updated if behavior changed
- TTS / STT / AEC impact has been considered
- backward compatibility has been considered
- no scope creep was introduced
