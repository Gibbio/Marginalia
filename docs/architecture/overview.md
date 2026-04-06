# Architecture Overview

## Shape

Marginalia is a lightweight modular monolith organized as a monorepo.

- `packages/core` owns domain concepts, events, ports, the session state
  machine, and application services
- `packages/adapters` owns fake and future concrete providers
- `packages/infra` owns SQLite, config, logging, and event bus wiring
- `apps/cli` owns the first usable interface and composition root

This remains effectively a clean or hexagonal architecture with low ceremony.

## Why This Shape

The early product needs strong boundaries more than it needs scale mechanics.
The architecture therefore optimizes for:

- clarity of domain vocabulary
- replaceable infrastructure
- low-cost local iteration
- future reuse by a desktop shell or local API

## Runtime Model

The current runtime is simple but real:

1. the CLI composes a local container
2. `DocumentIngestionService`, `ReaderService`, `NoteService`,
   `RewriteService`, `SummaryService`, `SearchService`, and
   `SessionQueryService` coordinate domain workflows
3. ports abstract command STT, dictation STT, synthesis, playback, rewrite,
   summarization, storage, and event publishing
4. `SQLiteDatabase` runs sequential file-based migrations (numbered `.sql`
   files tracked in a `schema_migrations` table), uses WAL mode, and applies
   a busy timeout for concurrent reader/writer safety
5. SQLite repositories persist documents, normalized sections and chunks,
   sessions, notes, rewrite drafts, and playback-related session metadata
6. real local Kokoro (default) or Piper TTS, Vosk command STT, and subprocess
   playback adapters can be selected through config, while deterministic fake
   adapters remain available
7. an in-process event bus publishes standardized domain events
8. the read-while-listen runtime is driven by a step-driven `RuntimeLoop`
   whose `step()` function returns a `StepStatus` — the caller owns the loop
   driver (CLI `while` loop, desktop timer, or async wrapper)

## What Is Implemented Now

- document ingestion with markdown heading and sentence-aware chunk parsing
  (configurable `chunk_target_chars`)
- normalized document persistence for documents, sections, and chunks
- session creation and persistence with explicit `is_active` flag
- persisted provider metadata and playback runtime metadata for the active
  session
- real local Kokoro synthesis to WAV artifacts by default, Piper as optional
  alternate adapter
- real local command recognition through Vosk when configured
- real local note dictation through whisper.cpp when configured
- real local playback through `afplay` when configured
- step-driven runtime loop decoupled from the CLI
- interactive shell (`marginalia shell`) with background RuntimeLoop thread
- signal handling for graceful shutdown during playback
- pause and resume state transitions
- repeat, rewind, chapter restart, chapter advance, stop, help, and bounded
  voice-command loop commands
- automatic chunk and chapter progression until completion or stop
- background pre-synthesis of the next chunk to eliminate inter-chunk TTS gaps
- reading progress tracking (section/chunk fractions, overall progress)
- note capture lifecycle with anchored notes
- rewrite draft generation through a deterministic fake provider
- topic summary generation through a deterministic fake provider
- local document and note search
- database and provider diagnostics through `doctor`
- session and playback projection reporting through `status`
- sequential file-based SQLite migrations
- audio cache cleanup with configurable max age
- structured logging with optional file handler
- `make setup` bootstraps the full stack in one command

## What Is Still Stubbed

- persistent event history or out-of-process subscribers
- draft review workflows beyond generation
- sentence-level playback tracking

## Why CLI First

CLI-first is not an aesthetic choice. It keeps the first implementation honest:

- session transitions can be defined before UI complexity exists
- provider contracts can be exercised without desktop decisions
- tests can target deterministic commands and outputs
- the same service graph can later back a desktop shell

## Why Monorepo

The product is small enough that repo fragmentation would increase coordination
cost immediately. A monorepo makes cross-cutting changes to docs, CLI, core,
and infra cheap while keeping boundaries explicit in the folder structure.

## Why Python Core

Python is the practical choice for:

- local AI tooling and model integration
- text manipulation
- CLI ergonomics
- future provider ecosystem compatibility

It is also fast enough for the current control-plane work, while the hot-path
audio or model details can stay isolated behind adapters later.

## Why SQLite Now

SQLite is sufficient for early persistence requirements:

- single-user local workflow
- low operational overhead
- transactional storage for documents, sessions, notes, and drafts
- easy backup and inspection
- a clear path from bootstrap schema to explicit migrations later

It is intentionally not treated as a forever constraint, but it is the right
starting point for a local-first product.
