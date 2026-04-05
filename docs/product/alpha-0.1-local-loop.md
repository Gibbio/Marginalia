# Alpha 0.1 Local Loop

## Goal

Alpha 0.1 proves the narrow local loop on macOS Apple Silicon:

1. ingest a markdown or text document
2. synthesize the current reading chunk with a local TTS provider
3. play the generated audio locally
4. recognize a small Italian command vocabulary locally
5. update persisted reading session state coherently

The supported real-provider path is:

- TTS: Kokoro by default, Piper as an optional alternate adapter
- playback: `afplay`
- command STT: Vosk with a constrained Italian grammar

## What Is Real

- document ingestion into SQLite
- persisted reading session state and provider metadata
- Kokoro synthesis to local WAV artifacts
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
- a Kokoro-compatible Python runtime on Python 3.12 or 3.11
- Python packages `kokoro` and `soundfile` available in that Kokoro runtime
- optionally `espeak-ng` available on `PATH` for better non-English coverage
- a local Vosk Italian model directory
- Python packages `vosk` and `sounddevice` available in the active environment
- microphone permission granted to the terminal app you use

`afplay` ships with macOS and is the expected playback command for this alpha.

## Configuration

Start from [examples/alpha-local-config.toml](/Users/mauriziogobbo/Marginalia/examples/alpha-local-config.toml).

Important notes:

- `default_voice` is the active Kokoro voice id in the default setup.
- `kokoro.python_executable` should point to a dedicated Python 3.12 or 3.11 runtime.
- `kokoro.lang_code = "i"` selects the Italian pipeline.
- `piper.model_path` remains available if you want to switch back to Piper.
- `vosk.commands` should stay small and explicit for this alpha.
- `providers.allow_fallback = false` is recommended for real alpha runs so missing prerequisites fail clearly.

Because the official `kokoro` package is smaller than the model weights it uses,
first synthesis may trigger a model download from Hugging Face. This is an
inference from the official package and model distribution, so treat the first
run as potentially networked unless the assets are already cached locally.

## Doctor

Run `doctor` first:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml doctor --json
```

Doctor reports:

- resolved paths
- configured provider names
- Kokoro runtime readiness
- Piper executable and model readiness
- Vosk model, Python package, and input-device readiness
- playback command readiness
- SQLite schema and table counts

Do not continue to the real alpha loop until:

- `provider_checks.kokoro.ready` is `true`
- `provider_checks.vosk.ready` is `true`
- `provider_checks.playback.ready` is `true`

For Vosk, `ready` now also requires at least one visible input audio device. A
Mac with only output devices will report Vosk as not ready even if the model
and Python packages are installed correctly.

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
- Fake providers remain available and stay useful for deterministic development and smoke flows.
