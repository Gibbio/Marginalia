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
- SQLite-backed local persistence with schema v2 compatibility bootstrap
- normalized document storage for documents, sections, and chunks
- real local Piper TTS and Vosk command-STT adapters behind ports
- local subprocess-backed playback for generated audio artifacts
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
- for the real alpha path on macOS: a local `piper` executable, a Piper model
  file, a Vosk model directory, and Python packages `vosk` plus `sounddevice`

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
  Piper, `afplay`, and Vosk

Example:

```bash
.venv/bin/python -m marginalia_cli --config examples/local-config.toml doctor --json
```

`doctor` reports:

- resolved local paths
- configured provider names
- provider capability metadata
- readiness checks for Piper, Vosk, and playback
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
- `listen`
- `control-loop`
- `note-start`
- `note-stop`
- `rewrite-current`
- `summarize-topic`
- `search-document`
- `search-notes`
- `status`
- `doctor`

Example V0 flow:

```bash
.venv/bin/python -m marginalia_cli ingest examples/sample-document.txt --json
.venv/bin/python -m marginalia_cli play --json
.venv/bin/python -m marginalia_cli repeat --json
.venv/bin/python -m marginalia_cli next-chapter --json
.venv/bin/python -m marginalia_cli pause --json
.venv/bin/python -m marginalia_cli control-loop --max-commands 2 --json
.venv/bin/python -m marginalia_cli note-start --json
.venv/bin/python -m marginalia_cli note-stop --text "Review the opening paragraph." --json
.venv/bin/python -m marginalia_cli rewrite-current --json
.venv/bin/python -m marginalia_cli summarize-topic local --json
.venv/bin/python -m marginalia_cli status --json
```

Example Alpha 0.1 real-provider flow:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml doctor --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml ingest path/to/document.md --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml play --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml listen --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml control-loop --max-commands 5 --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml status --json
```

## What Is Real Now

- document ingestion into SQLite with section and chunk parsing
- normalized document persistence for documents, sections, and chunks
- persisted reading session state changes across separate CLI invocations
- persisted provider metadata, audio references, and playback process metadata
- real local Piper synthesis to WAV artifacts when configured
- real local Vosk command recognition with a constrained Italian vocabulary when configured
- local subprocess-backed playback control through `play`, `pause`, `resume`, and `stop`
- bounded CLI voice command handling through `listen` and `control-loop`
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
- sentence-level playback tracking and automatic chapter progression
- desktop UI and editor adapters

## Roadmap Summary

Near term:

- extend schema bootstrap into explicit migrations
- improve chunking and reading progress heuristics
- add document, note, and draft inspection commands
- harden the real-provider path for more environments and better diagnostics

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
