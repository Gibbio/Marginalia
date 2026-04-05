# Development Setup

## Baseline

Marginalia is developed as a Python monorepo with a CLI-first workflow.

Recommended prerequisites:

- Python 3.12+
- `make`
- optional: VS Code with the recommended Python and Ruff extensions

## Bootstrap

```bash
make bootstrap
```

This creates `.venv`, upgrades `pip`, and installs the project plus development
dependencies in editable mode.

If you want a persistent local configuration file instead of environment
variables:

- start from `examples/local-config.toml` for the deterministic fake-provider path
- start from `examples/alpha-local-config.toml` for the real Alpha 0.1 local loop

## Daily Commands

```bash
make format
make lint
make test
make smoke
make run-cli-help
```

## Configuration

Useful environment variables:

- `MARGINALIA_HOME`
- `MARGINALIA_DATA_DIR`
- `MARGINALIA_DB_PATH`
- `MARGINALIA_AUDIO_CACHE_DIR`
- `MARGINALIA_LOG_LEVEL`
- `MARGINALIA_CONFIG`
- `MARGINALIA_FAKE_COMMANDS`
- `MARGINALIA_FAKE_DICTATION_TEXT`
- `MARGINALIA_DEFAULT_VOICE`
- `MARGINALIA_TTS_PROVIDER`
- `MARGINALIA_COMMAND_STT_PROVIDER`
- `MARGINALIA_PLAYBACK_PROVIDER`
- `MARGINALIA_ALLOW_PROVIDER_FALLBACK`
- `MARGINALIA_PIPER_EXECUTABLE`
- `MARGINALIA_PIPER_MODEL_PATH`
- `MARGINALIA_VOSK_MODEL_PATH`

The CLI `doctor` command reports the effective local configuration.

Example:

```bash
.venv/bin/python -m marginalia_cli --config examples/local-config.toml doctor --json
```

`doctor` is currently the fastest way to validate:

- resolved paths
- writable database location
- active provider names
- provider capabilities
- Piper, Vosk, and playback readiness
- SQLite schema version, profile, and current table counts

## Real Provider Setup

Alpha 0.1 targets macOS Apple Silicon and expects:

- a local `piper` executable plus a Piper `.onnx` voice model
- a local Vosk Italian model directory
- Python packages `vosk` and `sounddevice`
- microphone permission for the terminal app

The repository does not download these assets automatically. Configure them
explicitly through `examples/alpha-local-config.toml` or equivalent
environment variables, then verify readiness with `doctor`.

## Smoke Flow

`make smoke` exercises the current reference workflow:

- doctor
- ingest
- play
- repeat
- next-chapter
- restart-chapter
- pause
- resume
- control-loop
- note-start
- note-stop
- rewrite-current
- summarize-topic
- search-document
- search-notes
- status

This flow is deterministic and uses fake providers by default, including a
scripted voice-command loop.

## Devcontainer

A lightweight devcontainer is included so work can resume quickly on another
machine. It intentionally mirrors the local setup instead of introducing a
separate orchestration layer.

## Home And Office Development

The product is expected to evolve across mixed contexts:

- home: better for architecture, docs, ADRs, deeper model thinking, provider research
- office: better for bounded implementation, test hardening, CI fixes, and routine polishing

This distinction is reflected in the backlog seed with explicit context tags.
