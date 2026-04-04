# ADR 0002: Python Core

- Status: Accepted

## Context

Marginalia needs strong text handling, practical CLI ergonomics, local AI
ecosystem compatibility, and fast iteration. The early product is more about
orchestration, provider integration, and domain clarity than about extreme
runtime throughput.

## Decision

Implement the core in Python.

## Consequences

- provider experimentation is easier
- CLI development is straightforward
- the core remains accessible for future contributors
- performance-sensitive work, if needed later, must stay isolated behind clear
  boundaries

## Alternatives Considered

- TypeScript: strong tooling, but weaker fit for local AI and speech ecosystem work
- Rust: attractive for performance, but too expensive for the first iteration
