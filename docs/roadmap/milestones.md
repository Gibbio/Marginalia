# Roadmap Milestones

## Foundation

Goal: establish the repository and architectural baseline.

Status on April 4, 2026: substantially complete.

- monorepo structure
- docs and ADRs
- runnable CLI scaffolding
- SQLite schema bootstrap and repositories
- fake providers behind ports
- CI and development environment
- schema health reporting
- unit and integration test baseline

## V0 CLI Skeleton

Goal: cover the intended command surface with explicit local behavior.

Status on April 4, 2026: implemented as a usable skeleton.

- document ingestion
- session lifecycle commands
- note anchoring
- rewrite draft placeholder generation
- topic summarization placeholder generation
- search commands
- doctor and status diagnostics
- end-to-end smoke flow

Remaining hardening before V1:

- migration strategy beyond schema bootstrap
- richer chunking and progress semantics
- better note review and draft inspection commands
- persistent event history if it becomes operationally useful

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
