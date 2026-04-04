# Marginalia

Marginalia is a local AI-first voice reading and annotation engine. It is intended to read long-form text aloud, react to voice commands, capture dictated notes anchored to the current reading location, and later help rewrite or summarize sections of a document.

The repository is intentionally bootstrapped as a production-minded monorepo rather than a prototype. The first usable interface is a CLI, the core is Python, storage starts with SQLite, and all speech and LLM capabilities are designed behind replaceable ports.

## Why it exists

Reading, listening, annotating, and revising are still split across too many tools. Marginalia is meant to collapse those workflows into a local-first engine that can:

- read a document like an audiobook
- react to simple spoken control commands
- attach dictated notes to the exact place where they were spoken
- later use those notes to rewrite or summarize relevant sections

## Current Scope

The current repository focuses on foundation work:

- monorepo structure for long-term product development
- Python core packages with clean architecture boundaries
- CLI as the first operational interface
- SQLite-backed local persistence placeholders
- fake STT, TTS, playback, and LLM adapters behind ports
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

- `packages/core` contains domain models, state machine, application services, events, and ports
- `packages/adapters` contains replaceable fake provider implementations
- `packages/infra` contains configuration, logging, event bus, and SQLite storage
- `apps/cli` contains the user-facing command-line application

The core never depends on editor APIs, concrete speech providers, or remote services. Those concerns sit behind ports and can be added later without distorting the domain model.

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

More detail is documented in `docs/architecture/repository-structure.md`.

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

## Current CLI Commands

The CLI is intentionally small but installable and structured for growth:

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

Implemented now:

- document ingestion into SQLite
- reading session state changes
- anchored note capture via explicit text or fake dictation
- document and note search
- doctor and status reporting

Stubbed intentionally:

- real audio playback
- real speech recognition
- real summarization and rewrite generation

## Roadmap Summary

Near term:

- stabilize the core domain and state transitions
- define SQLite schema v0 and migration strategy
- harden the CLI into a usable local workflow
- add better tests around sessioning, notes, and storage

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

A final license has not been chosen yet. This repository includes [`LICENSE.placeholder`](LICENSE.placeholder) until a deliberate licensing decision is made.
