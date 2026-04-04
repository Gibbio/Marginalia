# ADR 0006: Editor Integration Deferred

- Status: Accepted

## Context

Future editor integrations are important, but the core domain is not stable
enough yet to bind it to any specific editor or plugin model. Doing so now would
risk turning editor constraints into core product constraints.

## Decision

Defer editor integrations, including Obsidian, until the local core contracts
are more mature.

## Consequences

- the core remains editor-agnostic
- future adapters must target exported contracts rather than internal domain code
- short-term workflow stays focused on CLI and local persistence

## Alternatives Considered

- build an Obsidian integration immediately: rejected because it would bias the
  architecture too early
- ignore editors completely: rejected because editor handoff remains a real future use case
