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
4. `SQLiteDatabase` bootstraps a stable SQLite v2 schema and applies
   compatibility upgrades for older bootstrap databases
5. SQLite repositories persist documents, normalized sections and chunks,
   sessions, notes, rewrite drafts, and playback-related session metadata
6. real local Piper, Vosk, and subprocess playback adapters can be selected
   through config, while deterministic fake adapters remain available
7. an in-process event bus publishes standardized domain events

## What Is Implemented Now

- document ingestion with markdown heading and paragraph chunk parsing
- normalized document persistence plus fallback support for older outline-only
  rows
- session creation and persistence
- persisted provider metadata and playback runtime metadata for the active
  session
- real local Piper synthesis to WAV artifacts when configured
- real local command recognition through Vosk when configured
- real local playback through `afplay` when configured
- pause and resume state transitions
- repeat, chapter restart, chapter advance, stop, and bounded voice-command
  loop commands
- note capture lifecycle with anchored notes
- rewrite draft generation through a deterministic fake provider
- topic summary generation through a deterministic fake provider
- local document and note search
- database and provider diagnostics through `doctor`
- session and playback projection reporting through `status`

## What Is Still Stubbed

- real note dictation STT
- persistent event history or out-of-process subscribers
- draft review workflows beyond generation
- sentence-level playback tracking and automatic chapter progression
- an explicit migration runner beyond the current schema bootstrap and
  compatibility logic

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
