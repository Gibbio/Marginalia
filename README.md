# Marginalia

Marginalia is a local AI-first voice reading and annotation engine. It is meant
to read long-form text aloud, react to voice-oriented controls, capture notes
anchored to the current reading location, and later help rewrite or summarize
sections of a document.

The repository is intentionally structured as a production-minded monorepo. The
first usable interface is a CLI, the core is Python, storage starts with
SQLite, and speech plus LLM capabilities stay behind replaceable ports.

## Why It Exists

Reading, listening, annotating, and revising are still split across too many
tools. Marginalia is meant to collapse those workflows into a local-first
engine that can:

- read a document like an audiobook
- react to simple spoken control commands later
- attach dictated notes to the exact place where they were spoken
- turn those notes into rewrites or summaries later

## Current Scope

As of April 5, 2026, the repository delivers Alpha 0.1 of the local reading
loop:

- monorepo structure for long-term product development
- Python core packages with clean architecture boundaries
- CLI as the first usable interface
- SQLite-backed local persistence with schema v3 compatibility bootstrap
- normalized document storage for documents, sections, and chunks
- real local Kokoro TTS by default, optional Piper TTS, and Vosk command-STT adapters behind ports
- local subprocess-backed playback for generated audio artifacts
- one supported runtime mode: `play` ingests or selects a file, starts reading automatically, opens the microphone automatically, and keeps command listening active until completion or explicit stop
- language-specific voice command lexicons loaded from TOML files
- stale runtime/process cleanup before a new foreground reading session starts
- fake provider fallbacks for testing and development
- event-driven application services for ingestion, sessioning, notes, rewrite,
  summary, search, and voice control
- CI, devcontainer, smoke flow, and updated architecture documentation

## Non-Goals For Now

- real desktop application implementation
- concrete editor integrations such as Obsidian
- network-distributed architecture or microservices
- real note dictation STT
- real rewrite or summarization provider integrations
- HTTP or WebSocket APIs

## Architecture Summary

Marginalia is structured as a lightweight modular monolith:

- `packages/core` contains domain models, the session state machine,
  application services, events, and provider/storage ports
- `packages/adapters` contains deterministic fake provider implementations and
  later real provider adapters
- `packages/infra` contains configuration, logging, event bus wiring, and
  SQLite repositories
- `apps/cli` contains the Typer CLI and the composition root

The core never depends on editor APIs, concrete speech SDKs, or remote service
contracts. Those concerns sit behind ports and can be replaced later without
distorting the domain model.

## Repository Layout

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

More detail lives in `docs/architecture/repository-structure.md`.

## Local Setup

Prerequisites:

- Python 3.12+
- `make`
- for the real alpha path on macOS: a Kokoro Python runtime compatible with
  Python `<3.13`, `afplay`, a Vosk model directory, and Python packages `vosk`
  plus `sounddevice`

Bootstrap a local environment:

```bash
make bootstrap
```

Useful commands:

```bash
make format
make lint
make test
make smoke
make run-cli-help
```

Real alpha instructions live in
[`docs/product/alpha-0.1-local-loop.md`](docs/product/alpha-0.1-local-loop.md).

## Configuration

Marginalia can run from environment variables or from an explicit TOML file.

- `examples/local-config.toml` keeps the deterministic fake-provider setup
- `examples/alpha-local-config.toml` shows the real local alpha path with
  Kokoro, `afplay`, and Vosk using the default OS audio devices

Example:

```bash
.venv/bin/python -m marginalia_cli --config examples/local-config.toml doctor --json
```

`doctor` reports:

- resolved local paths
- configured provider names
- configured command language and lexicon path
- provider capability metadata
- readiness checks for Kokoro, Piper, Vosk, and playback
- default input/output device visibility for the supported runtime path
- SQLite schema version, profile, tables, and row counts

## Current CLI Commands

The CLI surface currently includes:

- `ingest`
- `play`
- `pause`
- `resume`
- `repeat`
- `restart-chapter`
- `next-chapter`
- `stop`
- `note-start`
- `note-stop`
- `rewrite-current`
- `summarize-topic`
- `search-document`
- `search-notes`
- `status`
- `doctor`

Example Alpha 0.1 flow:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml doctor --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml play path/to/document.md --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml pause --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml resume --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml next-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml stop --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml status --json
```

## Single Runtime Mode

Alpha 0.1 now supports one interactive runtime only:

- provide a file path or stored document id to `play`
- Marginalia ingests the file when needed
- playback starts automatically on the OS default output device
- microphone listening starts automatically on the OS default input device
- command listening stays active while playback is ongoing
- the document advances chunk by chunk and chapter by chapter until the end or `stop`
- starting a new `play` cleans up any stale Marginalia runtime process first

The command vocabulary is loaded from language-specific TOML files under
`packages/infra/src/marginalia_infra/config/commands/`.

The repeatable manual verification flow lives in
[`docs/testing/alpha-0.1-runtime-loop.md`](docs/testing/alpha-0.1-runtime-loop.md).

## What Is Real Now

- document ingestion into SQLite with section and chunk parsing
- normalized document persistence for documents, sections, and chunks
- persisted reading session state changes across separate CLI invocations
- persisted provider metadata, audio references, and playback process metadata
- persisted runtime metadata for command listening, command language, runtime pid, and startup cleanup summary
- real local Kokoro synthesis to WAV artifacts when configured
- optional real local Piper synthesis to WAV artifacts when configured
- real local Vosk command recognition with a constrained language-specific command lexicon when configured
- local subprocess-backed playback control through the continuous `play` runtime plus manual `pause`, `resume`, and `stop`
- automatic chunk and chapter progression during the supported read+listen runtime
- startup cleanup of stale foreground runtime records before a new session begins
- anchored note capture via explicit text or a fake dictation provider
- deterministic rewrite draft generation with source anchor and provider
  metadata
- deterministic topic summarization with highlights and provider metadata
- document and note search over local storage
- `doctor` reporting for config, provider capabilities, and schema health
- `status` reporting for session, playback projection, note counts, and latest
  draft/note context
- end-to-end smoke flow covering ingest, play, navigation, note capture,
  rewrite, summary, search, and status

## What Is Still Stubbed

- real note dictation STT
- production rewrite and summarization providers
- persistent event history outside the current process
- sentence-level playback tracking
- true ducking during simultaneous Bluetooth playback and microphone capture
- desktop UI and editor adapters

## Roadmap Summary

Near term:

- extend schema bootstrap into explicit migrations
- improve chunking and reading progress heuristics
- add document, note, and draft inspection commands
- harden the real-provider path for more environments and better diagnostics
- evaluate whether the single read-while-listening runtime is smooth enough to pursue beyond Alpha 0.1

Later:

- desktop shell
- local API
- editor adapters
- real local or hybrid speech and LLM providers

See `docs/roadmap/milestones.md` and `docs/roadmap/backlog-seed.md`.

## Development Philosophy

- local-first before networked
- explicit architecture before speculative frameworks
- fake adapters behind ports before real provider lock-in
- documentation and ADRs as part of the product surface
- small coherent changes with tests and docs updates

## License

A final license has not been chosen yet. This repository includes
[`LICENSE.placeholder`](LICENSE.placeholder) until a deliberate licensing
decision is made.
