# Marginalia — AI Context Document

This document is a comprehensive briefing for any AI assistant working on the
Marginalia codebase. It is designed to be self-contained: read this before
touching any code.

Last updated: April 2026 (pre-Alpha 0.3).

---

## What Marginalia Is

A local AI-first voice reading and annotation engine. It reads long-form text
aloud, reacts to spoken commands, captures notes anchored to the current
reading position, and can later rewrite or summarize sections using those
notes.

The product is CLI-first, Python-based, SQLite-backed, and runs entirely on
the user's machine. There are no servers, no cloud APIs, no network
dependencies in the core path.

## Who Maintains It

Single developer (Maurizio Gobbo). The codebase is authored with AI assistance
and is expected to continue that way. Code quality, architecture discipline,
and clear documentation are non-negotiable.

## Repository Location and Stack

- GitHub: `Gibbio/Marginalia`
- Language: Python 3.12+
- CLI framework: Typer
- Storage: SQLite with WAL mode
- TTS: Kokoro (default), Piper (optional)
- Command STT: Vosk
- Playback: `afplay` via subprocess
- Testing: pytest
- Linting: ruff
- Type checking: mypy (strict mode)
- Build: setuptools with editable install

---

## Architecture

Lightweight hexagonal / clean architecture. Low ceremony, strong boundaries.

### Package Map

```
packages/core/src/marginalia_core/
    domain/         # Value objects, entities, enums — no dependencies
    events/         # DomainEvent, EventName enum
    ports/          # Protocol classes (interfaces) for all external concerns
    application/    # Services, command router, OperationResult
        services/   # ReaderService, RuntimeLoop, NoteService, etc.

packages/adapters/src/marginalia_adapters/
    fake/           # Deterministic test/dev adapters (FakePlaybackEngine, etc.)
    real/           # Real local adapters (Kokoro, Piper, Vosk, subprocess)

packages/infra/src/marginalia_infra/
    config/         # AppSettings, TOML loading, voice command lexicon files
    storage/        # SQLite repositories, migrations
    runtime/        # FileRuntimeSupervisor (PID tracking, file locking)
    logging/        # Logging setup
    events.py       # InMemoryEventBus

apps/cli/src/marginalia_cli/
    main.py         # Typer CLI entry point
    bootstrap.py    # Composition root — builds the entire object graph
```

### Dependency Rule

The core package NEVER imports from adapters, infra, or cli. All external
concerns are behind Protocol classes in `ports/`. The CLI's `bootstrap.py` is
the only place where concrete implementations are wired together.

### Key Protocols (ports/)

| Port | File | Purpose |
|---|---|---|
| `SpeechSynthesizer` | `tts.py` | Text-to-speech synthesis |
| `CommandRecognizer` | `stt.py` | Voice command recognition |
| `DictationTranscriber` | `stt.py` | Free-form dictation |
| `PlaybackEngine` | `playback.py` | Audio playback control |
| `RewriteGenerator` | `llm.py` | Section rewriting |
| `TopicSummarizer` | `llm.py` | Topic summarization |
| `DocumentRepository` | `storage.py` | Document persistence |
| `SessionRepository` | `storage.py` | Session persistence |
| `NoteRepository` | `storage.py` | Note persistence |
| `RewriteDraftRepository` | `storage.py` | Draft persistence |
| `RuntimeSupervisor` | `runtime.py` | Process lifecycle tracking |
| `EventPublisher` | `events.py` | Domain event publishing |

### Key Domain Models (domain/)

| Model | File | Role |
|---|---|---|
| `Document` | `document.py` | Ingested text with sections and chunks |
| `DocumentSection` | `document.py` | Chapter-level subdivision |
| `DocumentChunk` | `document.py` | Smallest addressable reading unit |
| `ReadingSession` | `reading_session.py` | Mutable session state (the central entity) |
| `ReaderState` | `reading_session.py` | High-level state enum (IDLE, READING, PAUSED, ...) |
| `PlaybackState` | `reading_session.py` | Low-level playback enum (STOPPED, PLAYING, PAUSED) |
| `ReadingPosition` | `reading_session.py` | section_index / chunk_index / char_offset |
| `VoiceNote` | `note.py` | Anchored note with transcript and position |
| `RewriteDraft` | `rewrite.py` | Generated rewrite for a section |

### Key Application Services (application/services/)

| Service | Purpose |
|---|---|
| `ReaderService` | Session lifecycle: play, pause, resume, stop, repeat, chapter navigation, voice command dispatch |
| `RuntimeLoop` | Step-driven read-while-listen loop (`step()` returns `StepStatus`) |
| `ReadingRuntimeService` | Thin wrapper that creates and drives a `RuntimeLoop` |
| `DocumentIngestionService` | Parse and persist documents |
| `NoteService` | Note capture lifecycle |
| `RewriteService` | Generate rewrite drafts |
| `SummaryService` | Generate topic summaries |
| `SearchService` | Search documents and notes |
| `SessionQueryService` | Status and session reporting |

### Service Return Convention

All services return `OperationResult(status, message, data)` where status is
`ok`, `planned`, or `error`. This makes CLI JSON output uniform. Never raise
exceptions for business logic errors — return `OperationResult.error(...)`.

### Event System

In-process `InMemoryEventBus`. Events are `DomainEvent(name, payload)` with
stable `EventName` enum values. Key events: `READING_STARTED`,
`READING_COMPLETED`, `PLAYBACK_STOPPED`, `COMMAND_DISPATCHED`,
`DOCUMENT_INGESTED`, `NOTE_SAVED`.

---

## Runtime Model

### The Read-While-Listen Loop

The central runtime is `RuntimeLoop` in `runtime_loop.py`. It works as a step
function:

```python
loop = RuntimeLoop(...)
loop.start(target)
with loop:              # opens microphone monitor
    while loop.step() is StepStatus.CONTINUE:
        pass
result = loop.finalize()
```

Each `step()` call:
1. Synchronizes the session with playback state
2. Advances to the next chunk if current playback finished
3. Captures the next voice command if the microphone is open
4. Dispatches the command through `ReaderService`

The caller owns the loop driver — the CLI uses a `while` loop with signal
handlers, a desktop app would use a timer.

### Voice Command Dispatch

Dict-driven dispatch table in `ReaderService._build_intent_dispatch_table()`.
Adding a new voice command requires exactly three changes:
1. New member in `VoiceCommandIntent` enum (`command_router.py`)
2. Phrases in each language TOML file (`config/commands/it.toml`, `en.toml`)
3. Entry in the dispatch table (`reader_service.py`)

Unhandled intents return an explicit error — they do NOT fall through to stop.

### Runtime Supervision

`FileRuntimeSupervisor` persists the active runtime as a JSON file with:
- PID and OS process start time (for PID reuse protection)
- Session and document IDs
- Advisory file locking via `fcntl.flock` to prevent races

On startup, stale runtimes are cleaned up: dead PIDs are removed, live PIDs
with matching start times are terminated, mismatched start times (PID reuse)
are skipped.

### Session Auto-Expiry

Sessions with `is_active=1` that haven't been updated in
`session_max_inactive_hours` (default 24h) are deactivated on startup in
`bootstrap.py`.

---

## SQLite Schema

Sequential file-based migrations in `infra/storage/migrations/`:
- `001_baseline.sql` — all tables
- `002_active_session_flag.sql` — `is_active` column

Current schema version: v4 (`sqlite-v4-migrated`).

Key tables: `documents`, `document_sections`, `document_chunks`, `sessions`,
`notes`, `drafts`, `schema_migrations`, `schema_metadata`.

WAL mode and `busy_timeout = 5000` are set on every connection.

---

## Configuration

`AppSettings` in `infra/config/settings.py` loads from:
1. Environment variables (`MARGINALIA_*`)
2. TOML config file (passed via `--config` or `MARGINALIA_CONFIG`)
3. Hardcoded defaults

Key settings:
- `command_language` — voice command language (`it`, `en`)
- `kokoro.default_voice` — Kokoro voice ID
- `kokoro.python_executable` — separate Python 3.12 runtime for Kokoro
- `vosk.model_path` — Vosk model directory
- `whisper_cpp.executable` — whisper.cpp binary path
- `whisper_cpp.model_path` — GGML model file path
- `providers.allow_fallback` — whether to fall back to fake when real fails
- `session_max_inactive_hours` — session expiry threshold (default 24)
- `audio_cache_max_age_hours` — audio cache cleanup (default 72)

---

## Testing

106 tests. All deterministic, no network, no real audio.

```bash
make test          # or: .venv/bin/python -m pytest tests/ -x -q
make lint          # ruff check + mypy
make smoke         # end-to-end with fake providers
```

Tests use `FakePlaybackEngine`, `FakeCommandRecognizer`, etc. The fake
adapters are first-class — they implement the same ports as real providers.

Test locations:
- `tests/unit/` — component tests
- `tests/integration/` — workflow tests
- `tests/fixtures/` — sample documents

### Before Committing

Always run: pytest, ruff, mypy. All three must pass clean.

---

## Code Conventions

- **Dataclasses** with `frozen=True, slots=True` for immutable value objects
- **Mutable dataclasses** with `slots=True` for `ReadingSession`
- **Protocol classes** for all ports (not ABC)
- **`from __future__ import annotations`** in every file
- **Type annotations** on every function signature
- **No ORMs** — raw SQL in repository classes
- **No global state** — everything is passed through constructors
- **ruff** for formatting (double quotes, 100 char line length)
- **mypy strict** mode (`disallow_untyped_defs`, `warn_return_any`, etc.)
- Line length: 100 characters
- Import order: stdlib, third-party, local (enforced by ruff)

### What NOT To Do

- Do not add dependencies to the core package on adapters, infra, or CLI
- Do not use `Any` in port signatures without justification
- Do not add ORM layers or dependency injection frameworks
- Do not add network calls in the core path
- Do not mock the database in tests — use real SQLite with `tmp_path`
- Do not add speculative abstractions for hypothetical requirements
- Do not skip type annotations or `# type: ignore` without a comment
- Do not add `import-untyped` to mypy ignores — CI uses `import-not-found`

---

## What Is Real vs Stubbed

### Real (working end-to-end)

- Document ingestion and SQLite persistence
- Session state machine with full transition coverage
- Kokoro TTS synthesis to WAV files
- Piper TTS synthesis (optional alternate)
- Vosk voice command recognition with language-specific grammar
- Subprocess playback via `afplay`
- Read-while-listen runtime loop
- Voice command dispatch (pause, resume, repeat, rewind, chapter nav, stop, help)
- Reading progress tracking in status responses and events
- Note capture with position anchoring
- whisper.cpp dictation transcriber for real note dictation
- Audio cache management
- Runtime supervision with PID reuse protection and file locking
- Session auto-expiry
- `doctor` and `status` diagnostics

### Stubbed (fake providers, real interfaces)

- Rewrite generation (returns deterministic draft)
- Topic summarization (returns deterministic summary)
- Desktop UI
- Editor adapters
- Persistent event history
- Sentence-level playback tracking

---

## Roadmap — What Comes Next

From `docs/roadmap/backlog-seed.md`, roughly in priority order:

1. **Document inspection commands** — let users list/inspect documents, notes,
   drafts from the CLI without opening SQLite
2. **Chunking improvements** — more deliberate than paragraph-only splitting
3. **Single runtime UX evaluation** — is Bluetooth listening + continuous
   command capture good enough?
4. **Real provider packaging** — reduce setup friction, improve `doctor` hints
5. **Event payload contracts** — stabilize before adding persistence
6. **Draft review workflow** — list, retrieve, transition draft status
7. **Command recognition robustness** — safe spoken variants, small grammar

---

## File Quick Reference

| What you need | Where to look |
|---|---|
| CLI entry point | `apps/cli/src/marginalia_cli/main.py` |
| Object graph wiring | `apps/cli/src/marginalia_cli/bootstrap.py` |
| Session state machine | `packages/core/src/marginalia_core/domain/reading_session.py` |
| Voice command routing | `packages/core/src/marginalia_core/application/command_router.py` |
| Reader service (dispatch, play, stop) | `packages/core/src/marginalia_core/application/services/reader_service.py` |
| Runtime loop (step function) | `packages/core/src/marginalia_core/application/services/runtime_loop.py` |
| All port definitions | `packages/core/src/marginalia_core/ports/` |
| Domain events | `packages/core/src/marginalia_core/events/models.py` |
| Real Kokoro adapter | `packages/adapters/src/marginalia_adapters/real/kokoro.py` |
| Real Vosk adapter | `packages/adapters/src/marginalia_adapters/real/vosk.py` |
| Fake adapters | `packages/adapters/src/marginalia_adapters/fake/` |
| SQLite repositories | `packages/infra/src/marginalia_infra/storage/sqlite.py` |
| SQL migrations | `packages/infra/src/marginalia_infra/storage/migrations/` |
| Runtime supervisor | `packages/infra/src/marginalia_infra/runtime/session_supervisor.py` |
| App settings | `packages/infra/src/marginalia_infra/config/settings.py` |
| Voice command lexicons | `packages/infra/src/marginalia_infra/config/commands/` |
| Example config | `examples/alpha-local-config.toml` |
| Changelog | `CHANGELOG.md` |
| Architecture docs | `docs/architecture/` |
| ADRs | `docs/adr/` |
| Backlog | `docs/roadmap/backlog-seed.md` |
| Milestones | `docs/roadmap/milestones.md` |

---

## How To Verify Your Changes

```bash
.venv/bin/python -m pytest tests/ -x -q       # all tests pass
.venv/bin/python -m ruff check apps/ packages/ tests/  # no lint errors
.venv/bin/python -m mypy apps/cli/src packages/core/src packages/adapters/src packages/infra/src tests  # no type errors
```

If you add a new voice command intent, also verify:
- The intent enum member exists in `VoiceCommandIntent`
- At least one phrase exists in every language TOML file
- The dispatch table in `ReaderService` handles it
- A test covers the new intent

If you modify the SQLite schema, add a new numbered migration file in
`storage/migrations/` — never modify existing migration files.

If you add a new port, add both a fake adapter and a real adapter (or document
why only one exists).
