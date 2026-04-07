# ADR 0003: CLI First

- Status: Superseded by ADR 0008

## Context

Marginalia eventually needs a richer user interface, but the first uncertain
problems are not visual. They are about session state, note anchoring, provider
contracts, and local workflow shape.

## Decision

Use a CLI as the first usable interface.

This ADR served the first product phase. Marginalia now uses a headless backend
plus client frontends, formalized in ADR 0008.

## Consequences

- the core can be exercised before desktop decisions are made
- tests and smoke flows are easier to automate
- the first frontend work can validate the service graph before stronger
  frontend/backend boundaries are introduced

## Alternatives Considered

- desktop-first: rejected because UI work would obscure the core workflow risks
- API-first: rejected because network surfaces are deferred on purpose
