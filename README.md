# Marginalia

Marginalia is a local AI-first voice reading and annotation engine. It reads
long-form text aloud, reacts to voice commands, captures notes anchored to the
current reading position, and can later help rewrite or summarize sections of a
document.

The repository is structured as a production-minded monorepo. The first usable
interface is a CLI, the core is Python, storage starts with SQLite, and speech
plus LLM capabilities stay behind replaceable ports.

## Why It Exists

Reading, listening, annotating, and revising are still split across too many
tools. Marginalia collapses those workflows into a local-first engine that can:

- read a document like an audiobook
- react to spoken control commands
- attach dictated notes to the exact place where they were spoken
- turn those notes into rewrites or summaries

## Installation

### 1. Base environment

Requirements: Python 3.12+ and `make`.

```bash
git clone https://github.com/Gibbio/Marginalia.git
cd Marginalia
make bootstrap
```

This creates a `.venv` virtualenv and installs the project in editable mode
with dev dependencies (mypy, pytest, ruff).

Running without any `--config` flag uses all-fake providers — no external
dependencies needed. This is enough for development, tests, and smoke flows.

### 2. Real providers (macOS Apple Silicon alpha path)

To run the full read-while-listening loop with real audio you need three
external providers. Each one is optional and falls back to fake if missing
(unless `allow_fallback = false` in your config).

#### Kokoro TTS (text-to-speech)

Kokoro requires its own Python 3.12 virtualenv because `kokoro` is not yet
compatible with Python 3.13+.

```bash
make bootstrap-kokoro
```

This creates `.venv-kokoro/` with `kokoro` and `soundfile`. The first
synthesis run may download model weights from Hugging Face.

Optionally install `espeak-ng` for better non-English phonemization:

```bash
brew install espeak-ng
```

#### Vosk command STT (speech-to-text)

Install the Python packages in the main virtualenv:

```bash
.venv/bin/pip install vosk sounddevice
```

Download an Italian model (or whichever language you need):

```bash
mkdir -p .models/vosk
cd .models/vosk
curl -LO https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip
unzip vosk-model-small-it-0.22.zip
cd ../..
```

Your terminal app must have microphone permission on macOS.

#### Playback

`afplay` ships with macOS — no additional installation needed.

#### Optional: Piper TTS

Piper is an alternative TTS adapter. Install the `piper` binary and download
an ONNX voice model. See `docs/product/alpha-0.1-local-loop.md` for details.

### 3. Configuration

Copy and edit the example config to match your local paths:

```bash
cp examples/alpha-local-config.toml my-config.toml
# edit my-config.toml — update kokoro.python_executable, vosk.model_path, etc.
```

Key settings:

| Setting | Purpose |
|---|---|
| `command_language` | Language for voice commands (`it`, `en`) |
| `kokoro.python_executable` | Path to the Kokoro Python 3.12 runtime |
| `kokoro.default_voice` | Kokoro voice id (e.g. `im_nicola`, `if_sara`) |
| `kokoro.lang_code` | Kokoro language pipeline (`i` for Italian) |
| `vosk.model_path` | Path to the Vosk model directory |
| `providers.allow_fallback` | Set to `false` for real alpha runs |

### 4. Verify

```bash
.venv/bin/python -m marginalia_cli --config my-config.toml doctor --json
```

Do not proceed to the real loop until `provider_checks.kokoro.ready`,
`provider_checks.vosk.ready`, and `provider_checks.playback.ready` are all
`true`.

## Quick Start

Fake-provider smoke flow (no external deps):

```bash
make smoke
```

Real alpha flow:

```bash
.venv/bin/python -m marginalia_cli --config my-config.toml play path/to/document.md --json
```

What `play` does:

- ingests the file if it is a path on disk
- starts playback automatically on the OS default output device
- opens the microphone automatically on the OS default input device
- keeps command listening active while reading
- advances chunk by chunk and chapter by chapter
- exits on document completion, explicit `stop`, or `Ctrl-C`
- cleans up any stale Marginalia runtime before starting

## CLI Commands

| Command | Description |
|---|---|
| `play` | Start or resume reading a document |
| `pause` | Pause playback |
| `resume` | Resume playback |
| `repeat` | Repeat the current chunk |
| `restart-chapter` | Restart the current chapter |
| `next-chapter` | Skip to the next chapter |
| `stop` | Stop the reading session |
| `status` | Show session state and playback info |
| `ingest` | Import a document without playing it |
| `note-start` | Begin capturing a note |
| `note-stop` | Stop capturing and anchor the note |
| `rewrite-current` | Generate a rewrite draft for the current section |
| `summarize-topic` | Generate a topic summary |
| `search-document` | Search document text |
| `search-notes` | Search captured notes |
| `doctor` | Report config, providers, and schema health |

## Voice Commands

The command vocabulary is loaded from language-specific TOML files under
`packages/infra/src/marginalia_infra/config/commands/`.

Italian (`it`):

| Command | Phrases |
|---|---|
| Pause | `pausa` |
| Resume | `continua`, `riprendi` |
| Repeat | `ripeti` |
| Next chapter | `capitolo successivo` |
| Restart chapter | `ricomincia capitolo` |
| Status | `stato` |
| Stop | `stop`, `ferma`, `fermati` |
| Help | `aiuto`, `comandi` |

## Architecture

Marginalia is a lightweight modular monolith:

- **`packages/core`** — domain models, session state machine, application
  services, events, and provider/storage ports
- **`packages/adapters`** — deterministic fake providers and real provider
  adapters (Kokoro, Piper, Vosk, subprocess playback)
- **`packages/infra`** — configuration, logging, event bus, SQLite
  repositories, and runtime supervision
- **`apps/cli`** — Typer CLI and composition root

The core never depends on editor APIs, concrete speech SDKs, or remote service
contracts. Those concerns sit behind ports and can be replaced without
distorting the domain model.

```text
.
├── apps/
│   ├── cli/
│   └── desktop/
├── docs/
│   ├── adr/
│   ├── architecture/
│   ├── contributing/
│   ├── product/
│   ├── roadmap/
│   └── vision/
├── examples/
├── packages/
│   ├── adapters/
│   ├── core/
│   └── infra/
├── scripts/
└── tests/
```

More detail in `docs/architecture/repository-structure.md`.

## Development

```bash
make format      # ruff format + autofix
make lint        # ruff check + mypy
make test        # pytest
make smoke       # end-to-end smoke flow with fake providers
make run-cli-help
```

See `docs/contributing/development-setup.md` for more.

## Current State

As of April 2026, the repository delivers a pre-Alpha 0.3 local reading loop:

- SQLite-backed persistence with sequential file-based migrations (v4)
- real Kokoro TTS, optional Piper TTS, Vosk command STT behind ports
- step-driven runtime loop with automatic playback and command listening
- dict-driven voice command dispatch with file-driven lexicons per language
- PID reuse protection and advisory file locking on the runtime record
- session auto-expiry for stale sessions
- signal handling (SIGINT/SIGTERM) for graceful shutdown
- structured logging for provider selection, process cleanup, and command
  dispatch
- `READING_COMPLETED` and `COMMAND_DISPATCHED` domain events
- fake provider fallbacks for testing and development
- event-driven services for ingestion, sessioning, notes, rewrite, summary,
  search, and voice control

### Still stubbed

- real note dictation STT
- production rewrite and summarization providers
- persistent event history outside the current process
- sentence-level playback tracking
- desktop UI and editor adapters

## Roadmap

Near term: document inspection commands, chunking improvements, real-provider
packaging hardening, single runtime UX evaluation.

Later: desktop shell, local API, editor adapters, real local or hybrid speech
and LLM providers.

See `docs/roadmap/milestones.md` and `docs/roadmap/backlog-seed.md`.

## License

A final license has not been chosen yet. This repository includes
[`LICENSE.placeholder`](LICENSE.placeholder) until a deliberate licensing
decision is made.
