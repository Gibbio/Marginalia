# Alpha 0.1 Local Loop

## Goal

Alpha 0.1 proves the narrow local loop on macOS Apple Silicon:

1. ingest a markdown or text document
2. synthesize the current reading chunk with a local TTS provider
3. play the generated audio locally
4. recognize a small Italian command vocabulary locally
5. update persisted reading session state coherently

The supported real-provider path is:

- TTS: Piper CLI
- playback: `afplay`
- command STT: Vosk with a constrained Italian grammar

## What Is Real

- document ingestion into SQLite
- persisted reading session state and provider metadata
- Piper synthesis to local WAV artifacts
- local playback through a subprocess-backed adapter
- Vosk microphone capture for a bounded command vocabulary
- `listen` and `control-loop` CLI flows

## What Remains Fake

- note dictation STT
- rewrite generation
- topic summarization
- sentence-accurate playback progress
- automatic chapter progression when audio ends

## Prerequisites

- macOS on Apple Silicon
- Python 3.12+
- `make`
- a local `piper` executable available on `PATH`, or a configured absolute path
- a local Piper `.onnx` voice model file
- a local Vosk Italian model directory
- Python packages `vosk` and `sounddevice` available in the active environment
- microphone permission granted to the terminal app you use

`afplay` ships with macOS and is the expected playback command for this alpha.

## Configuration

Start from [examples/alpha-local-config.toml](/Users/mauriziogobbo/Marginalia/examples/alpha-local-config.toml).

Important notes:

- `piper.model_path` selects the actual voice model.
- `default_voice` is a session label and cache key, not a Piper model selector.
- `vosk.commands` should stay small and explicit for this alpha.
- `providers.allow_fallback = false` is recommended for real alpha runs so missing prerequisites fail clearly.

## Doctor

Run `doctor` first:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml doctor --json
```

Doctor reports:

- resolved paths
- configured provider names
- Piper executable and model readiness
- Vosk model and Python package readiness
- playback command readiness
- SQLite schema and table counts

Do not continue to the real alpha loop until:

- `provider_checks.piper.ready` is `true`
- `provider_checks.vosk.ready` is `true`
- `provider_checks.playback.ready` is `true`

## CLI Flow

Ingest a document:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml ingest path/to/document.md --json
```

Start playback for the latest or selected document:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml play --json
```

Control the active session with one-shot voice capture:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml listen --json
```

Or run a bounded voice loop:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml control-loop --max-commands 5 --json
```

Manual command equivalents remain available:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml pause --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml resume --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml repeat --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml next-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml restart-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml status --json
```

## Supported Voice Commands

- `pausa`
- `continua`
- `ripeti`
- `capitolo successivo`
- `ricomincia capitolo`
- `stato`
- `stop`

## Limitations

- Playback is chunk-based, not sentence-based.
- `resume` may re-synthesize the current chunk if there is no live paused playback process to continue.
- `listen` and `control-loop` are bounded CLI commands, not a background service.
- The microphone path is intentionally optimized for short deterministic commands, not free dictation.
- Fake providers remain available and are still the default unless configuration selects the real path.
