# ADR 0007: Lightweight Clean Architecture

- Status: Accepted

## Context

Marginalia needs strong boundaries between domain logic and concrete providers,
but it does not need a heavy enterprise framework or network-distributed
architecture. The product is still learning what the right workflows are.

## Decision

Use a lightweight clean or hexagonal architecture:

- clear ports and adapters
- explicit application services
- thin CLI orchestration
- minimal infrastructure wiring

## Consequences

- the codebase stays understandable
- future UI and provider surfaces can reuse the same core
- developers must resist unnecessary layering or speculative abstractions

## Alternatives Considered

- framework-heavy service architecture: rejected because it adds ceremony without solving current problems
- direct script-style code: rejected because it would not support long-term product growth
