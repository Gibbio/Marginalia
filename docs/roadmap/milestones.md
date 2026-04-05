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

## V0 CLI Skeleton

Goal: cover the intended command surface with explicit local behavior.

Status on April 5, 2026: implemented and hardened.

- document ingestion
- session lifecycle commands
- note anchoring
- rewrite draft placeholder generation
- topic summarization placeholder generation
- document and note search
- doctor and status diagnostics
- deterministic provider capability reporting
- normalized SQLite storage for documents, sections, and chunks
- end-to-end smoke flow including navigation and note flow

Remaining hardening before V1:

- explicit migrations beyond schema bootstrap
- richer chunking and progress semantics
- better document, note, and draft inspection commands
- a bounded command-listening loop for `LISTENING_FOR_COMMAND`
- optional event persistence if it becomes operationally useful

## V1 Usable CLI

Goal: make the CLI practical for a single-user local workflow.

- stronger document inspection and review commands
- more robust document chunking and progress semantics
- better note capture ergonomics
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
