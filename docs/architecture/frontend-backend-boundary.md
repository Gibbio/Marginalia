# Frontend / Backend Boundary

## Status

This document originally described the Alpha process boundary between frontend
clients and the Python backend.

For Beta, the durable boundary is no longer "frontend vs backend process". It
is "host shell vs shared engine".

The file is kept because the underlying contract ideas still matter.

## Beta Interpretation

In Beta, Marginalia should look like this:

```text
+---------------------------+
| Host Shells               |
| - desktop                 |
| - iOS                     |
| - Android                 |
| - tui-rs tool             |
+-------------+-------------+
              |
              v
+---------------------------+
| Shared Rust Engine        |
| - domain                  |
| - application services    |
| - contracts               |
| - storage rules           |
| - provider ports          |
+---------------------------+
```

## Rules That Still Hold

### 1. Hosts talk through contracts

Hosts should not import or depend on:

- repositories
- concrete providers
- SQLite internals
- runtime wiring details

They should depend on:

- command DTOs
- query DTOs
- event DTOs
- snapshots
- capability descriptions

### 2. The boundary is transport-neutral

The product contract should survive different embedding strategies:

- in-process host calls
- FFI
- transitional local transport layers

Transport is an adapter. The contract is the durable surface.

### 3. State remains engine-owned

The shared engine owns the meaning of:

- sessions
- reading position
- playback state
- note anchoring
- provider capabilities

Hosts render and control that state. They do not redefine it.

## What Changed Since Alpha

These Alpha assumptions are now obsolete as architecture targets:

- a headless Python backend is the authoritative runtime shape
- stdio is the permanent local boundary
- desktop-centric process supervision defines the product

Those patterns can still exist during migration, but only as transitional
hosting choices.

## Source Of Truth

Use these documents for Beta decisions:

- [`docs/architecture/overview.md`](/home/debian/sources/Marginalia/docs/architecture/overview.md)
- [`docs/architecture/beta-repository-structure.md`](/home/debian/sources/Marginalia/docs/architecture/beta-repository-structure.md)
- [`docs/adr/0011-beta-target-desktop-ios-android.md`](/home/debian/sources/Marginalia/docs/adr/0011-beta-target-desktop-ios-android.md)
- [`docs/adr/0013-adopt-rust-engine-with-native-host-shells.md`](/home/debian/sources/Marginalia/docs/adr/0013-adopt-rust-engine-with-native-host-shells.md)
