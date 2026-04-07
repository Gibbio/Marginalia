# ADR 0008: Headless Backend and Client Frontends

- Status: Accepted

## Context

Marginalia is no longer only targeting a CLI shell. The likely product surfaces
now include:

- a terminal UI
- a desktop GUI
- an Obsidian plugin
- future mobile or companion apps

If those surfaces call Python services directly, each new UI will recreate its
own orchestration, state projection, and long-running runtime handling. That
would make later UI work expensive and would leak product-specific constraints
back into the core.

Marginalia still does not want a distributed system or an HTTP service
architecture. It remains a local-first product with a single-user runtime.

## Decision

Treat Marginalia as a headless local backend with explicit frontend contracts.

The architecture is divided into:

- backend: domain workflows, runtime loop, storage, provider integration,
  orchestration, and authoritative state
- frontends: TUI, desktop GUI, editor plugins, and future mobile clients
- protocol: explicit versioned commands, queries, events, and snapshots

The protocol is transport-neutral. The first implementation should use local
IPC, not HTTP:

- required first transport: stdio with JSON Lines
- likely second transport: Unix domain socket with the same message envelopes
- deferred: HTTP APIs and network-exposed services

Frontends must depend only on exported contracts, not on repositories, services,
or domain objects.

## Backend Responsibilities

The backend owns:

- application workflow execution
- runtime lifecycle and long-running background work
- persistence and recovery
- provider selection and capability reporting
- authoritative session, playback, and note state
- event publication for client synchronization

The backend may expose multiple transports over time, but all of them must map
to the same contract layer.

## Frontend Responsibilities

Frontends own:

- rendering and interaction design
- local input history, focus, layout, and shortcut handling
- optimistic UX only when safe
- reconnect and resync behavior

Frontends do not own:

- reading-session truth
- runtime orchestration
- storage writes outside exported commands
- provider lifecycle logic

## Contract Shape

The frontend/backend boundary is defined in terms of:

- commands: state-changing requests
- queries: snapshot reads
- events: backend-to-frontend streaming updates
- snapshots: coherent state projections for initial load and resync
- capabilities: backend-declared feature and provider availability

The first stable contract should include at least:

- session control commands
- note commands
- document selection and ingestion commands
- read-only status and corpus queries
- runtime, playback, and note events
- a protocol version field

## Consequences

- TUI, GUI, Obsidian, and mobile clients can share one backend model
- the current CLI stops being the primary composition root over time
- DTOs and projections become first-class design artifacts
- transport code stays in infra or dedicated apps, not in the core domain
- some short-term UI work becomes slightly slower, but the rewrite cost drops
  sharply

## Non-Goals

This decision does not require:

- a public HTTP API
- multi-user concurrency
- remote cloud services
- embedding Python inside Rust or JavaScript clients

It also does not require every future platform to run the exact same Python
binary. The stable requirement is the contract boundary, not the process model
of each future host.

## Alternatives Considered

- continue with UI-specific direct service calls: rejected because it would
  duplicate orchestration and slow future product surfaces
- HTTP API first: rejected because it adds network semantics before they are
  needed
- embed backend logic into each frontend runtime: rejected because it would
  fragment the product model and increase rewrite cost
