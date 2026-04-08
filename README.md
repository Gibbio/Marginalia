# Marginalia

Marginalia is a local AI-first voice reading and annotation engine. It reads
long-form text aloud, reacts to voice commands, captures notes anchored to the
current reading position, and can later help rewrite or summarize sections of a
document.

The repository is structured as a production-minded monorepo. The backend is
Python, storage starts with SQLite, and speech plus LLM capabilities stay
behind replaceable ports while frontend clients are free to evolve separately.

The current Alpha implementation is desktop-first and macOS-oriented. The Beta
direction is broader: one product architecture that can be hosted on desktop,
iOS, and Android.

## Why It Exists

Reading, listening, annotating, and revising are still split across too many
tools. Marginalia collapses those workflows into a local-first engine that can:

- read a document like an audiobook
- react to spoken control commands
- attach dictated notes to the exact place where they were spoken
- turn those notes into rewrites or summaries

## Installation

### Quick setup (full stack)

Requirements: macOS Apple Silicon, Python 3.12+, `make`, Homebrew.

```bash
git clone https://github.com/Gibbio/Marginalia.git
cd Marginalia
make setup
```

This single command installs system dependencies (portaudio, espeak-ng, uv),
creates the Python environment, sets up all real providers (Kokoro TTS, Vosk
command STT, whisper.cpp dictation), downloads models, generates a starter
config, and runs `doctor` to verify everything works.

This setup reflects the current Alpha reference host. It is not the final Beta
packaging story for desktop, iOS, or Android.

Your terminal app must have microphone permission on macOS.

### Development-only setup (fake providers)

```bash
make bootstrap
```

Creates `.venv` and installs the project with dev dependencies. No external
deps needed — fake providers are used by default. Enough for development,
tests, and smoke flows.

### Manual provider setup

If you prefer to install providers individually:

```bash
make bootstrap-kokoro          # Kokoro TTS (separate Python 3.12 venv)
make bootstrap-vosk            # Download Vosk Italian model
make bootstrap-whisper         # Build whisper.cpp + download base model
make bootstrap-runtime-deps    # Install vosk, sounddevice, numpy in main venv
```

### Configuration

`make setup` generates `marginalia.toml` automatically. For manual config,
copy and edit the example:

```bash
cp examples/alpha-local-config.toml marginalia.toml
```

Key settings:

| Setting | Purpose |
|---|---|
| `command_language` | Language for voice commands (`it`, `en`) |
| `kokoro.python_executable` | Path to the Kokoro Python 3.12 runtime |
| `kokoro.default_voice` | Kokoro voice id (e.g. `im_nicola`, `if_sara`) |
| `kokoro.lang_code` | Kokoro language pipeline (`i` for Italian) |
| `vosk.model_path` | Path to the Vosk model directory |
| `whisper_cpp.executable` | Path to whisper.cpp binary |
| `whisper_cpp.model_path` | Path to GGML model file |
| `providers.allow_fallback` | Set to `false` for strict real-provider runs |

### Verify

```bash
make doctor
```

Do not proceed to the real loop until `provider_checks.kokoro.ready`,
`provider_checks.vosk.ready`, and `provider_checks.playback.ready` are all
`true`.

## Quick Start

### Rust TUI (recommended)

The TUI spawns the Python backend automatically as a child process (via
stdio JSON Lines) — there is no separate backend to start.

```bash
make tui-rs
```

This requires `marginalia.toml` in the project root.  `make setup`
generates it automatically.  If you skipped `make setup`, create it
manually:

```bash
cp examples/alpha-local-config.toml marginalia.toml
# edit paths inside to match your machine (python, models, etc.)
```

Without a valid `marginalia.toml` the backend falls back to fake
providers and you will not hear any audio.

You can also launch the TUI directly with a custom config path:

```bash
MARGINALIA_CONFIG=path/to/config.toml cargo run --manifest-path apps/tui-rs/Cargo.toml
```

Inside the TUI command bar:

```
/ingest path/to/document.md
/play
/note prova nota
/stop
```

TUI navigation highlights:

- empty command bar: arrow keys navigate reading position
- while typing: `Up` and `Down` navigate suggestions
- `Ctrl-C` must be pressed twice to quit

### Fake-provider smoke flow (no external deps)

```bash
make smoke
```

### Single-command real alpha flow (CLI)

```bash
.venv/bin/python -m marginalia_cli --config marginalia.toml play path/to/document.md --json
```

What `play` does:

- ingests the file if it is a path on disk
- starts playback automatically on the OS default output device
- opens the microphone automatically on the OS default input device
- keeps command listening active while reading
- pre-synthesizes the next chunk in the background to eliminate gaps
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
| `status` | Show session state and reading progress |
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
| Rewind | `indietro`, `precedente` |
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
- **`apps/backend`** — headless local backend and frontend contract transport
- **`apps/cli`** — thin Python CLI over the same backend composition root
- **`apps/tui-rs`** — Rust `ratatui + crossterm` frontend client

The core never depends on editor APIs, concrete speech SDKs, or remote service
contracts. Those concerns sit behind ports and can be replaced without
distorting the domain model.

```text
.
├── apps/
│   ├── backend/
│   ├── cli/
│   ├── desktop/
│   └── tui-rs/
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
make test        # pytest (106 tests)
make smoke       # end-to-end smoke flow with fake providers
make tui-rs      # Rust TUI frontend over backend stdio
make shell       # interactive Marginalia shell
make doctor      # check provider readiness
make run-cli-help
```

See `docs/contributing/development-setup.md` for more.

## Current State

As of April 2026, the repository delivers a pre-Alpha 0.3 local reading loop:

- SQLite-backed persistence with sequential file-based migrations (v4)
- real Kokoro TTS, optional Piper TTS, Vosk command STT, whisper.cpp
  dictation STT behind ports
- interactive shell with background RuntimeLoop thread
- step-driven runtime loop with automatic playback and command listening
- background pre-synthesis of the next chunk (eliminates inter-chunk gaps)
- sentence-aware chunking with configurable target size
- reading progress tracking (section/chunk fractions, overall progress)
- dict-driven voice command dispatch with file-driven lexicons per language
- PID reuse protection and advisory file locking on the runtime record
- session auto-expiry for stale sessions
- signal handling (SIGINT/SIGTERM) for graceful shutdown
- structured logging for provider selection, process cleanup, and command
  dispatch
- `READING_COMPLETED` and `COMMAND_DISPATCHED` domain events
- `make setup` bootstraps the full stack in one command
- fake provider fallbacks for testing and development
- 106 tests, clean lint and types

### Still stubbed

- production rewrite and summarization providers
- persistent event history outside the current process
- sentence-level playback tracking
- desktop UI and editor adapters

## Roadmap

Near term: document inspection commands, session management, doctor
remediation hints, note review and editing.

Later: EPUB/PDF ingestion, real LLM rewrite and summarization, desktop
shell upgrade, editor adapters.

See `docs/roadmap/milestones.md` and `docs/roadmap/backlog-seed.md`.

## License

A final license has not been chosen yet. This repository includes
[`LICENSE.placeholder`](LICENSE.placeholder) until a deliberate licensing
decision is made.
