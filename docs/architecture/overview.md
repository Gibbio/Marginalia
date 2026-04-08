# Architecture Overview

## Status

This document describes the active Beta architecture direction.

The repository currently contains:

- the Alpha Python implementation, still usable as a reference host and
  migration source
- the Beta Rust engine migration, now the target architecture

When Alpha-era documents disagree with this one, the Beta ADRs and migration
documents win.

## Target Shape

Marginalia Beta is a monorepo organized around a shared engine and
host-specific shells.

- `crates/` owns the shared Rust engine
- `apps/` owns desktop, iOS, Android, and tool hosts
- `models/` owns local AI asset layout
- `packages/` remains the Alpha Python reference during migration

This is still a modular monolith. The difference is that the durable center of
the product is no longer a Python backend process. It is the shared engine.

## Engine vs Host

The shared engine owns:

- domain models
- application services
- commands, queries, events, and snapshots
- provider and repository ports
- persistence rules
- reading-session and playback state semantics

Hosts own:

- UI
- audio session behavior
- device permissions
- app lifecycle
- OS integration
- concrete embedding of the shared engine

This is the main Beta boundary.

## Current Repository Reality

Today the repository is transitional.

The runnable Alpha loop still lives in Python:

- `packages/core`
- `packages/adapters`
- `packages/infra`
- `apps/backend`
- `apps/cli`

The new Beta engine work now starts in Rust:

- [`crates/marginalia-core`](/home/debian/sources/Marginalia/crates/marginalia-core)

The Rust TUI remains in
[`apps/tui-rs`](/home/debian/sources/Marginalia/apps/tui-rs) as a desktop
development and administration tool during migration.

## Boundary Principles That Still Hold

The following Alpha principles are still correct:

- keep domain logic independent from concrete providers
- keep playback, TTS, STT, storage, and LLM work behind ports
- keep SQLite as the initial local persistence layer
- treat commands, queries, events, and snapshots as explicit product contracts
- keep the repository as one monorepo

## What Changed For Beta

The following Alpha assumptions are no longer architectural targets:

- Python as the durable engine language
- a headless backend child process as the mandatory runtime shape
- stdio as the assumed long-term frontend boundary
- desktop and Unix process assumptions as the center of the product

The replacement direction is:

- Rust engine crates
- native or host-specific shells
- FFI or host embedding at the engine boundary
- Kokoro plus ONNX Runtime as the common local TTS direction

## Why This Shape

Beta must support one product model across:

- desktop
- iOS
- Android

That requires:

- a shared systems-level engine
- host-aware boundaries
- portable local persistence
- portable model/runtime planning
- the ability to keep UI and OS integration native where needed

## Source Of Truth

Read these together:

- [`NEXT.md`](/home/debian/sources/Marginalia/NEXT.md)
- [`docs/adr/0011-beta-target-desktop-ios-android.md`](/home/debian/sources/Marginalia/docs/adr/0011-beta-target-desktop-ios-android.md)
- [`docs/adr/0013-adopt-rust-engine-with-native-host-shells.md`](/home/debian/sources/Marginalia/docs/adr/0013-adopt-rust-engine-with-native-host-shells.md)
- [`docs/architecture/beta-repository-structure.md`](/home/debian/sources/Marginalia/docs/architecture/beta-repository-structure.md)
- [`docs/migration/alpha-to-beta-repo-mapping.md`](/home/debian/sources/Marginalia/docs/migration/alpha-to-beta-repo-mapping.md)
