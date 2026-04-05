# Architecture Overview

## Shape

Marginalia is a lightweight modular monolith organized as a monorepo.

- `packages/core` owns domain concepts, events, ports, state machine, and
  application services
- `packages/adapters` owns fake and future concrete providers
- `packages/infra` owns SQLite, config, logging, and event bus wiring
- `apps/cli` owns the first usable interface and composition root

This is effectively a clean or hexagonal architecture with low ceremony.

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
3. ports abstract speech, playback, storage, and LLM operations
4. a `SQLiteDatabase` bootstraps schema v0 and backs document, session, note,
   and draft repositories
5. fake adapters stand in for real providers with deterministic outputs
6. an in-process event bus publishes standardized domain events

## What Is Implemented Now

- document ingestion with markdown heading and paragraph chunk parsing
- session creation and persistence
- pause and resume state transitions
- note capture lifecycle with anchored notes
- rewrite draft generation through a fake provider
- topic summary generation through a fake provider
- local document and note search
- database health reporting through `doctor`

## What Is Still Stubbed

- actual audio playback
- microphone capture and speech recognition
- persistent event history or out-of-process subscribers
- review workflows for multiple rewrite drafts
- migration framework beyond schema version bootstrap

## Why CLI First

CLI-first is not an aesthetic choice. It keeps the first implementation honest:

- session transitions can be defined before UI complexity exists
- provider contracts can be exercised without desktop decisions
- tests can target deterministic commands and outputs
- the same service graph can later back a desktop shell

## Why Monorepo

The product is small enough that repo fragmentation would increase coordination
cost immediately. A monorepo makes cross-cutting changes to docs, CLI, core, and
infra cheap while keeping boundaries explicit in the folder structure.

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

It is intentionally not treated as a forever constraint, but it is the right
starting point for a local-first product.
