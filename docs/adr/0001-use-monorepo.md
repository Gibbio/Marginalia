# ADR 0001: Use Monorepo

- Status: Accepted

## Context

Marginalia needs a core domain, CLI, infrastructure, documentation, and future
product surfaces that will evolve together. Splitting those areas into separate
repositories now would increase coordination cost before there is enough stable
separation to justify it.

## Decision

Use a single monorepo with clear internal boundaries for apps, packages, docs,
tests, and automation.

## Consequences

- cross-cutting changes stay cheap
- docs and ADRs can evolve with code
- package boundaries must be enforced by convention and review
- future extraction remains possible if a boundary proves mature

## Alternatives Considered

- multiple repositories for core, CLI, and docs: rejected because it creates
  overhead too early
- flat single-package repository: rejected because it hides future boundaries
