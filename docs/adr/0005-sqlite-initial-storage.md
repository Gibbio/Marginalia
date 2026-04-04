# ADR 0005: SQLite Initial Storage

- Status: Accepted

## Context

The initial product is single-user and local-first. It needs reliable storage
for documents, sessions, notes, and draft placeholders without operational
overhead or infrastructure setup.

## Decision

Use SQLite as the initial persistence layer.

## Consequences

- local setup stays lightweight
- schema design becomes visible early
- migrations can stay simple in the first phase
- future scaling beyond a local single-user model would require a new decision

## Alternatives Considered

- PostgreSQL: rejected because it adds needless operational weight now
- file-only storage: rejected because queryability and state consistency matter
