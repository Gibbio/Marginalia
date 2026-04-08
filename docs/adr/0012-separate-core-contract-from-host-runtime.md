# ADR 0012: Separate Core Contract from Host Runtime

- Status: Accepted

## Context

Marginalia's current backend host is a Python process composed in
`apps/backend`, typically reached over local stdio transport. That works well
for the current Alpha desktop loop.

It is not a safe assumption for Beta hosts:

- iOS does not want a spawned Python child process
- Android does not want a spawned Python child process
- desktop shells may still use a local child process, but should not define the
  architecture for every host

The product already has a useful contract-oriented shape: commands, queries,
events, snapshots, ports, and application services. Beta needs to formalize
that these are the stable product boundary, while the concrete host runtime may
vary by platform.

## Decision

Treat Marginalia's command/query/event contract and application services as the
stable product engine boundary.

Treat the current Python backend process as one host implementation of that
boundary, not the canonical runtime shape for every platform.

This means:

- the core domain, service orchestration, and frontend contracts stay central
- host runtimes may be process-based, embedded, or otherwise platform-native
- transports such as stdio remain implementation choices, not product
  requirements
- process supervision, playback control, microphone capture, and file access
  should move behind host-aware abstractions instead of remaining tied to Unix
  process semantics

## Consequences

- Beta work should reduce dependence on PID files, Unix signals, advisory file
  locks, and subprocess-only playback behavior
- the current Python backend remains useful as the desktop reference host
- iOS and Android can adopt host-specific shells without redefining the product
  model
- tests should increasingly focus on contract and service behavior rather than
  only process wiring

## Non-Goals

This decision does not require:

- rewriting the core into another language immediately
- deleting the Python backend
- shipping a network service
- one identical transport on every host

It does require that new Beta architecture work distinguish between "engine"
and "current Python host".

## Alternatives Considered

- preserve the Python child-process backend as the mandatory architecture for
  every host: rejected because it is not a credible iOS or Android runtime
  story
- duplicate business logic separately inside each host shell: rejected because
  it would fragment the product model and slow every future change
