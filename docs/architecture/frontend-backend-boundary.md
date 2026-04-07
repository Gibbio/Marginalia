# Frontend / Backend Boundary

## Intent

Marginalia should support multiple frontends without rewriting reading logic,
runtime orchestration, or persistence rules. The boundary therefore treats every
UI as a client of a local headless backend.

Developer-facing implementation guidance lives in:

- [`docs/architecture/frontend-client-guide.md`](/home/debian/sources/Marginalia/docs/architecture/frontend-client-guide.md)

## Target Shape

```text
+------------------------+     +----------------------------------+
| Frontends              |     | Local Backend                    |
| - TUI                  | --> | - command/query gateway          |
| - Desktop GUI          | --> | - runtime loop + supervisors     |
| - Obsidian plugin      | --> | - application services           |
| - Future mobile client | --> | - storage + provider adapters    |
+------------------------+     +----------------------------------+
              ^                              |
              |                              v
              +------ commands / queries / events ------+
```

## Rules

### 1. The backend is authoritative

The backend owns the real state of:

- sessions
- playback
- note capture
- provider readiness
- runtime lifecycle

Every frontend must be able to reconnect and recover from backend snapshots.

### 2. Frontends talk only through contracts

Frontends must never import or directly call:

- repositories
- concrete providers
- SQLite code
- runtime supervisors
- application services

They may use:

- command DTOs
- query DTOs
- event DTOs
- snapshot DTOs
- capability DTOs

### 3. The boundary is transport-neutral

The message model should be identical whether the transport is:

- stdio JSON Lines
- Unix domain socket
- a future desktop bridge

The transport is an adapter. The contract is the product surface.

## Recommended Package Shape

### `packages/core`

Add a dedicated frontend contract layer under the application package:

- `marginalia_core.application.frontend.commands`
- `marginalia_core.application.frontend.queries`
- `marginalia_core.application.frontend.events`
- `marginalia_core.application.frontend.snapshots`
- `marginalia_core.application.frontend.capabilities`
- `marginalia_core.application.frontend.gateway`

This layer should expose typed DTOs and a backend-facing gateway protocol. It
should not know about stdio, sockets, TUI widgets, or GUI frameworks.

### `packages/infra`

Put transport adapters here:

- stdio server/client adapter
- Unix socket server/client adapter
- serialization helpers
- event fan-out or local subscription infrastructure

### `apps/backend`

This becomes the headless local process that:

- builds the container
- exposes the frontend gateway over a transport
- owns the backend process lifecycle

### `apps/cli`

This should evolve from “main product interface” into one of:

- a thin administrative shell
- a debug client
- a bootstrap helper

It should stop being the primary orchestration surface.

## Contract Categories

### Commands

Commands mutate backend state.

Examples:

- `ingest_document`
- `start_session`
- `pause_session`
- `resume_session`
- `stop_session`
- `repeat_chunk`
- `previous_chunk`
- `next_chapter`
- `create_note`
- `rewrite_current_section`

### Queries

Queries return coherent snapshots.

Examples:

- `get_app_snapshot`
- `get_session_snapshot`
- `list_documents`
- `get_document_view`
- `list_notes`
- `search_documents`
- `search_notes`
- `get_backend_capabilities`
- `get_doctor_report`

### Events

Events keep clients synchronized without polling every action.

Examples:

- `session_started`
- `session_progressed`
- `playback_state_changed`
- `note_capture_started`
- `note_saved`
- `runtime_stopped`
- `runtime_failed`
- `provider_warning_emitted`

## Snapshot Strategy

Each client should be able to render from a small number of stable snapshots.

Recommended initial snapshots:

- `AppSnapshot`
  High-level backend status, active session id, available capabilities.
- `SessionSnapshot`
  Reading state, position, playback state, active providers, runtime state.
- `DocumentSnapshot`
  Document metadata, sections, chunks, anchors, and current highlight target.
- `NotesSnapshot`
  Notes for the active or selected document.

Events should update these snapshots incrementally, but queries must always be
able to rebuild them from scratch.

## Capability Model

Different clients need different affordances. The backend should publish
capabilities such as:

- available transports
- available commands
- provider readiness
- whether dictation is enabled
- whether rewrite and summary providers are enabled

This lets a TUI, desktop shell, or Obsidian plugin degrade gracefully instead
of hard-coding assumptions.

## Implementation Path

1. Introduce frontend DTOs and a backend gateway in `packages/core`.
2. Adapt the current CLI entrypoints to call that gateway rather than raw
   services.
3. Add a stdio transport in `packages/infra`.
4. Introduce `apps/backend` as the headless backend process.
5. Move future TUI, GUI, and plugin work onto the contract layer.
6. Add Unix socket transport only when a second class of client actually needs
   it.

## Guardrails

- no HTTP surface unless a product requirement explicitly demands it
- no frontend-specific types in the core domain
- no transport logic in application services
- no client bypass around the backend for storage or provider access
- protocol messages must be versioned from the start
