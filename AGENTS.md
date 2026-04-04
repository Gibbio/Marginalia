# Agent Guidance

## Purpose

Marginalia is a local AI-first voice reading and annotation engine. The current
goal is not feature breadth; it is a durable core that can support months of
incremental product work.

## Architectural Constraints

- Keep the repository as a single monorepo.
- Keep Python as the core implementation language.
- Treat the CLI as the first-class interface until a later desktop shell exists.
- Keep SQLite as the initial persistence layer.
- Keep STT, TTS, playback, and LLM integrations behind ports.
- Do not couple the core to Obsidian or any specific editor.
- Do not introduce microservices or distributed runtime assumptions.
- Do not add HTTP or WebSocket APIs unless explicitly requested later.

## Working Rules

- Inspect the relevant package boundaries before editing.
- Prefer small coherent commits over broad mixed changes.
- Update tests and docs in the same change when behavior changes.
- Update or add ADRs for architectural changes.
- Keep fake providers explicit and isolated behind ports.
- Prefer straightforward code and docstrings over speculative abstractions.

## What Not To Do

- Do not wire domain logic directly to CLI, desktop, or future editor APIs.
- Do not bypass ports by calling concrete providers from the core.
- Do not silently introduce remote-first assumptions or background services.
- Do not treat placeholders as finished implementations.
- Do not add a real editor integration in the core.

## Documentation Expectations

- Product intent belongs in `docs/vision/` and `docs/product/`.
- Architecture decisions belong in `docs/adr/`.
- Structural or workflow changes must update the relevant docs.
- README changes should keep setup and scope accurate.

## Future Provider Work

- Real STT/TTS/LLM providers are acceptable only behind existing or newly added
  ports.
- Provider configuration must remain explicit and local-first.
- If a provider requires network access, document that clearly and keep the
  domain model independent from it.

## Task Structuring

- Core changes: start in `packages/core`.
- Adapter changes: implement in `packages/adapters` or `packages/infra`.
- CLI changes: keep orchestration in `apps/cli`, not business logic.
- Architecture changes: add or update an ADR before merging.
