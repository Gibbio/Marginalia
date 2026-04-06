# Alpha 0.1 Local Loop

> **Historical document.** This describes the Alpha 0.1 state completed in
> early April 2026. The current state is pre-Alpha 0.3 — see
> `docs/roadmap/milestones.md` and `NEXT.md` for up-to-date status.

## Goal

Alpha 0.1 proves the narrow local loop on macOS Apple Silicon:

1. ingest a markdown or text document
2. start reading automatically with a local TTS provider
3. open the microphone automatically on the OS default input device
4. keep command listening active while playback is ongoing
5. update persisted reading session state coherently until document completion or stop
6. restart safely by cleaning up a stale previous runtime before a new session begins

The supported real-provider path is:

- TTS: Kokoro by default, Piper as an optional alternate adapter
- playback: `afplay`
- command STT: Vosk with a constrained language-specific command lexicon

## What Is Real

- document ingestion into SQLite
- persisted reading session state and provider metadata
- persisted runtime metadata for active listening, command language, runtime pid, and startup cleanup
- Kokoro synthesis to local WAV artifacts
- local playback through a subprocess-backed adapter
- Vosk microphone capture for a bounded command vocabulary loaded from file
- automatic reading + automatic continuous command listening in the `play` runtime
- automatic chunk and chapter progression until completion or stop

## What Remains Fake

- note dictation STT
- rewrite generation
- topic summarization
- sentence-accurate playback progress

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

Start from [examples/alpha-local-config.toml](examples/alpha-local-config.toml).

Important notes:

- `command_language = "it"` selects the lexicon file used for spoken command matching.
- command phrases come from `packages/infra/src/marginalia_infra/config/commands/<language>.toml`.
- `kokoro.default_voice` is the active Kokoro voice id.
- `kokoro.python_executable` should point to a dedicated Python 3.12 or 3.11 runtime.
- `kokoro.lang_code = "i"` selects the Italian pipeline.
- `piper.model_path` remains available if you want to switch back to Piper.
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
- configured command language and command lexicon file
- Kokoro runtime readiness
- Piper executable and model readiness
- Vosk model, Python package, and default-input readiness
- playback command readiness
- default input and output audio device visibility when `sounddevice` is available
- SQLite schema and table counts

Do not continue to the real alpha loop until:

- `provider_checks.kokoro.ready` is `true`
- `provider_checks.vosk.ready` is `true`
- `provider_checks.playback.ready` is `true`

For Vosk, `ready` now also requires at least one visible input audio device. A
Mac with only output devices will report Vosk as not ready even if the model
and Python packages are installed correctly.

## CLI Flow

Start the only supported runtime flow:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml play path/to/document.md --json
```

What `play` now does:

- ingests the file when you pass a path
- starts playback automatically
- starts command listening automatically
- keeps listening active while reading
- advances across chunks and chapters automatically
- exits only on document completion, explicit `stop`, or fatal runtime failure
- cleans up a stale prior Marginalia runtime before starting

Manual command equivalents remain available:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml pause --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml resume --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml repeat --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml next-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml restart-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml status --json
```

## Supported Voice Commands (Italian)

- `pausa`
- `continua` / `riprendi`
- `ripeti`
- `capitolo successivo`
- `ricomincia capitolo`
- `stato`
- `stop` / `ferma` / `fermati`
- `aiuto` / `comandi`

## Limitations

- Playback is chunk-based, not sentence-based.
- `resume` may re-synthesize the current chunk if there is no live paused playback process to continue.
- Bluetooth audio quality may still degrade at the macOS device-profile level when the same headset is used for both playback and microphone input.
- The microphone path is intentionally optimized for short deterministic commands, not free dictation.
- Fake providers remain available and stay useful for deterministic development and smoke flows.

For the repeatable manual runtime verification flow, use
[docs/testing/alpha-0.1-runtime-loop.md](docs/testing/alpha-0.1-runtime-loop.md).
