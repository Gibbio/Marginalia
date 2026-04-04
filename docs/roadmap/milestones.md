# Roadmap Milestones

## Foundation

Goal: establish the repository and architectural baseline.

- monorepo structure
- docs and ADRs
- CLI scaffolding
- SQLite placeholder storage
- fake providers behind ports
- CI and development environment

## V0 CLI Skeleton

Goal: cover the intended command surface with explicit local behavior.

- document ingestion
- session lifecycle commands
- note anchoring
- search commands
- doctor and status diagnostics

## V1 Usable CLI

Goal: make the CLI practical for a single-user local workflow.

- stronger session persistence
- more robust document chunking
- better note capture ergonomics
- local schema migration strategy
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
