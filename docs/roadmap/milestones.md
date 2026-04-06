# Roadmap Milestones

## Foundation

Goal: establish the repository and architectural baseline.

Status on April 5, 2026: complete.

- monorepo structure
- docs and ADRs
- runnable CLI scaffolding
- SQLite schema foundation and repositories
- fake providers behind ports
- CI and development environment
- schema and provider health reporting
- unit and integration test baseline

## Alpha 0.1 Local Reading Loop

Goal: prove a narrow but real local reading loop on macOS Apple Silicon.

Status on April 5, 2026: complete — superseded by Alpha 0.2.

- document ingestion
- session lifecycle commands with persisted provider/runtime metadata
- real local Kokoro synthesis path by default, with Piper retained as an optional alternate adapter
- real local Vosk command-recognition path
- local subprocess playback path
- one supported foreground runtime: `play` starts reading plus continuous command listening together
- language-specific command lexicon files loaded from TOML
- stale runtime cleanup before a new foreground session starts
- note anchoring
- rewrite draft placeholder generation
- topic summarization placeholder generation
- document and note search
- doctor and status diagnostics
- deterministic provider capability reporting
- normalized SQLite storage for documents, sections, and chunks
- end-to-end smoke flow including navigation, note flow, and scripted command loop

## Alpha 0.2 Desktop-Ready Infrastructure

Goal: harden the runtime model, persistence layer, and infrastructure so the
core is ready to drive a desktop shell without requiring further architectural
rework.

Status on April 5, 2026: implemented.

- step-driven `RuntimeLoop` decoupled from the CLI — can be driven by a CLI
  loop, a desktop timer, or an async wrapper
- sequential file-based SQLite migration system replacing the old bootstrap-and-patch approach
- explicit `is_active` session flag instead of implicit ordering
- SQLite WAL mode and busy timeout for concurrent reader/writer safety
- connection caching to avoid repeated open/close overhead
- signal handling (SIGINT/SIGTERM) for graceful shutdown during playback
- audio cache cleanup with configurable max age
- structured logging with optional file handler
- `ReadingPosition.from_anchor()` deduplication across adapters
- dead code removal (StorageCoordinator, inline schema constant, duplicated helpers)

## Pre-Alpha-0.3 Hardening

Goal: make the runtime trustworthy enough for Alpha 0.3 by reducing risk in
session lifecycle, process cleanup, command extensibility, provider
swappability, and observability.

Status on April 5, 2026: implemented.

- fixed implicit fallback-to-stop in voice command dispatch — unhandled intents
  now return an explicit error instead of silently stopping playback
- refactored voice command dispatch from if/elif chain to a dict-driven table —
  adding a new intent requires only an enum member, a TOML entry, and a dict
  entry
- added `HELP` voice command intent with Italian (`aiuto`, `comandi`) and
  English (`help`, `commands`) phrases
- added stop aliases: `fermati` (Italian) and `halt` (English)
- added `READING_COMPLETED` and `COMMAND_DISPATCHED` domain events so
  completion is distinguishable from stop in the event stream
- added structured logging to: process cleanup (supervisor), provider selection
  (bootstrap), command dispatch (reader service), session start/stop/completion
  (reader service and runtime loop)
- added logging to bootstrap provider selection so fallback decisions are
  visible and auditable
- added 12 new tests covering: completion vs stop distinction, help intent,
  alias resolution, restart after completion, status truthfulness, provider
  capability reporting, fallback visibility
- PID reuse protection: runtime supervisor records OS process start time and
  skips termination when start time mismatches
- advisory file locking (`fcntl.flock`) on the runtime JSON to prevent
  concurrent CLI races
- session auto-expiry: stale `is_active=1` sessions are deactivated on startup
  when they exceed `session_max_inactive_hours` (default 24 h)
- `_handled_commands` list capped at 1000 entries in `RuntimeLoop` to prevent
  unbounded memory growth
- 5 additional tests: session expiry, PID reuse protection, file locking
  non-deadlock, command cap enforcement
- sentence-aware chunking: long paragraphs split at punctuation, short ones
  merged into reading-sized units (configurable `chunk_target_chars`)
- reading progress tracking: `progress` dict in status/sync/events with
  section and chunk fractions plus overall `chunks_read/total_chunks`
- `REWIND` voice command intent (go back one chunk, crossing sections)
- chapter boundary logging in auto-advance
- whisper.cpp dictation transcriber adapter with config, doctor check,
  bootstrap wiring, and `make bootstrap-whisper`
- 10 chunking tests, 10 progress and rewind tests, 9 whisper tests
- total test count: 94

Remaining hardening before V1:

- better document, note, and draft inspection commands
- stronger real-provider install ergonomics and compatibility guidance
- optional event persistence if it becomes operationally useful
- playback speed cycling, inter-chunk pause, context re-read on resume

## V1 Usable CLI

Goal: make the CLI practical for a single-user local workflow.

- stronger document inspection and review commands
- more robust document chunking and progress semantics
- better real note capture ergonomics
- explicit local schema migration strategy
- clearer rewrite and summary review flow

## V2 Desktop Shell

Goal: add a thin desktop interface without changing core assumptions.

- desktop shell spike
- service reuse from the CLI composition root
- local playback and note UX experiments

## V3 Editor Integration Spike

Goal: evaluate editor adapters after the core contracts stabilize.

- export contracts for notes and rewrite drafts
- adapter spike for an editor such as Obsidian
- explicit boundary validation so the core stays editor-agnostic
