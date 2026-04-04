# ADR 0003: CLI First

- Status: Accepted

## Context

Marginalia eventually needs a richer user interface, but the first uncertain
problems are not visual. They are about session state, note anchoring, provider
contracts, and local workflow shape.

## Decision

Use a CLI as the first usable interface.

## Consequences

- the core can be exercised before desktop decisions are made
- tests and smoke flows are easier to automate
- the future desktop shell should reuse the same service graph rather than
  invent parallel logic

## Alternatives Considered

- desktop-first: rejected because UI work would obscure the core workflow risks
- API-first: rejected because network surfaces are deferred on purpose
