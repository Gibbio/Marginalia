# Marginalia

Marginalia is a local AI-first voice reading and annotation engine. It is meant
to read long-form text aloud, react to voice-oriented controls, capture dictated
notes anchored to the current reading location, and later help rewrite or
summarize sections of a document.

The repository is intentionally structured as a production-minded monorepo. The
first usable interface is a CLI, the core is Python, storage starts with
SQLite, and speech plus LLM capabilities stay behind replaceable ports.

## Why It Exists

Reading, listening, annotating, and revising are still split across too many
tools. Marginalia is meant to collapse those workflows into a local-first engine
that can:

- read a document like an audiobook
- react to simple spoken control commands later
- attach dictated notes to the exact place where they were spoken
- turn those notes into rewrites or summaries later

## Current Scope

As of April 4, 2026, the repository covers a real Foundation plus V0 CLI
skeleton:

- monorepo structure for long-term product development
- Python core packages with clean architecture boundaries
- CLI as the first usable interface
- SQLite-backed local persistence with schema bootstrap and health checks
- fake STT, TTS, playback, and LLM adapters behind ports
- event-driven application services for ingestion, sessioning, notes, rewrite,
  summary, and search
- architecture, roadmap, ADR, and contribution documentation
- CI, devcontainer, and engineering hygiene

## Non-Goals For Now

- real desktop application implementation
- concrete editor integrations such as Obsidian
- network-distributed architecture or microservices
- production STT/TTS integrations
- HTTP or WebSocket APIs

## Architecture Summary

Marginalia is structured as a lightweight modular monolith:

- `packages/core` contains domain models, state machine, application services,
  events, and ports
- `packages/adapters` contains replaceable fake provider implementations
- `packages/infra` contains configuration, logging, event bus wiring, and
  SQLite storage
- `apps/cli` contains the user-facing command-line application

The core never depends on editor APIs, concrete speech providers, or remote
services. Those concerns sit behind ports and can be added later without
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

## Configuration

Marginalia can run from environment variables or from an explicit TOML file. A
sample configuration is available at `examples/local-config.toml`.

Example:

```bash
.venv/bin/python -m marginalia_cli --config examples/local-config.toml doctor --json
```

## Current CLI Commands

The CLI surface currently includes:

- `ingest`
- `play`
- `pause`
- `resume`
- `repeat`
- `restart-chapter`
- `next-chapter`
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
.venv/bin/python -m marginalia_cli pause --json
.venv/bin/python -m marginalia_cli note-start --json
.venv/bin/python -m marginalia_cli note-stop --text "Review the opening paragraph." --json
.venv/bin/python -m marginalia_cli rewrite-current --json
.venv/bin/python -m marginalia_cli summarize-topic local --json
.venv/bin/python -m marginalia_cli status --json
```

## What Is Real Now

- document ingestion into SQLite with section and chunk parsing
- persisted reading session state changes
- anchored note capture via explicit text or a fake dictation provider
- deterministic rewrite draft generation through a fake provider
- deterministic topic summarization through a fake provider
- document and note search over local storage
- doctor and status reporting with schema and database details
- end-to-end smoke flow covering ingest, play, pause, note, rewrite, summary,
  search, and status

## What Is Still Stubbed

- actual audio playback
- microphone capture and speech recognition
- production rewrite and summarization providers
- persistent event history outside the current process
- desktop UI and editor adapters

## Roadmap Summary

Near term:

- extend the SQLite bootstrap into explicit migrations
- improve chunking and reading progress heuristics
- add richer note and draft inspection commands
- prepare the core for a thin desktop shell without changing boundaries

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
