# Frontend Client Guide

## Status

This document is now mainly historical.

It records the client model used by the Alpha Python backend and the current
Rust TUI. It is not the source of truth for the Beta architecture.

For Beta, the active boundary is:

- shared Rust engine
- host-specific shells
- FFI or host embedding instead of a mandatory backend child process

Read these first instead:

- [`docs/adr/0013-adopt-rust-engine-with-native-host-shells.md`](/home/debian/sources/Marginalia/docs/adr/0013-adopt-rust-engine-with-native-host-shells.md)
- [`docs/architecture/overview.md`](/home/debian/sources/Marginalia/docs/architecture/overview.md)
- [`docs/architecture/beta-repository-structure.md`](/home/debian/sources/Marginalia/docs/architecture/beta-repository-structure.md)

## What Still Matters

The Alpha contract work is still useful because it established:

- explicit commands
- explicit queries
- explicit snapshots
- explicit capability reporting

Those concepts survive into Beta even though the transport and runtime shape
will change.

## Alpha Reference Transport

The current TUI still talks to the Python backend over stdio JSON Lines.

Relevant code:

- [`apps/tui-rs`](/home/debian/sources/Marginalia/apps/tui-rs)
- [`apps/backend`](/home/debian/sources/Marginalia/apps/backend)
- [`packages/core/src/marginalia_core/application/frontend`](/home/debian/sources/Marginalia/packages/core/src/marginalia_core/application/frontend)

Treat that path as a transitional reference host, not the permanent app model.
