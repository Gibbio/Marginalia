# Marginalia — Beta Next

This document resets planning after the current Alpha work. The Alpha proved
that Marginalia can be a real local reading engine. Beta must reshape that
engine so it can become one product family across desktop, iOS, and Android.

Last updated: April 2026.

---

## Beta Direction

Marginalia Beta targets:

- desktop
- iOS
- Android

Windows is explicitly not a Beta priority.

The Beta goal is not "more Alpha features". The Beta goal is:

- one portable product model
- one shared Rust engine
- one portable local TTS direction
- host-aware runtime boundaries
- platform-specific shells over shared engine contracts

## What We Keep From Alpha

These Alpha decisions still look correct and should be preserved:

- modular monorepo
- Python as the current reference implementation during migration
- SQLite as the local persistence layer
- explicit ports for TTS, STT, playback, storage, runtime, and LLM features
- command/query/event contract between engine and clients
- document, chunk, anchor, and reading-session domain model
- local-first product stance

Alpha already produced a useful engine:

- real read-while-listen runtime
- document ingestion and chunking
- session persistence
- note anchoring
- backend/frontend contract
- deterministic tests and fake providers

That work is not being discarded. Beta starts from it, but stops treating the
current macOS-centric Python host as the end state.

## What Beta Must Change

The current Alpha host shape still assumes too many desktop and Unix details:

- Python child-process backend as the default runtime model
- stdio as the only serious local transport
- subprocess playback built around desktop commands
- process supervision based on PID files, signals, and file locking
- real provider choices tied to desktop Python environments

Those assumptions are acceptable for Alpha and wrong for Beta planning.

## Beta Priorities

## Priority 1 — Freeze the Product Boundary

The engine contract must become the stable center of the product.

Deliverables:

- command/query/event/snapshot contract reviewed for Beta completeness
- DTOs treated as canonical app-facing types
- no new feature work that bypasses the contract
- explicit distinction between engine behavior and host behavior

Exit condition:

- desktop, iOS, and Android can all be described as hosts of the same engine
  model, even if their concrete runtimes differ

## Priority 2 — Separate Engine from Host Runtime

Beta needs a host-aware runtime architecture.

Deliverables:

- identify process-bound assumptions in playback, supervision, transport, and
  lifecycle management
- move those assumptions behind host-specific interfaces
- define the desktop reference host separately from future mobile hosts
- stop treating the current Python backend process as mandatory architecture

Exit condition:

- the core engine can be described without assuming stdio, PID management, or
  Unix signals

## Priority 3 — Build the Shared Rust Engine

Beta needs one durable shared implementation language for the engine itself.

Deliverables:

- Rust becomes the target language for shared engine crates
- FFI boundary defined for desktop, iOS, and Android hosts
- Alpha Python code treated as migration reference, not Beta center

Exit condition:

- the new shared engine ownership is visible in repository structure and first
  migration slices

## Priority 4 — Standardize on Kokoro + ONNX Runtime

Beta needs one credible local TTS path across desktop and mobile.

Deliverables:

- Kokoro remains the canonical TTS model family
- ONNX Runtime becomes the target inference runtime
- Python Kokoro worker is treated as transitional desktop implementation only
- packaging, cache, and model versioning strategy documented

Exit condition:

- desktop and mobile can converge on one TTS direction instead of forking into
  separate provider families

## Priority 5 — Rebuild Desktop as the Reference Product Host

Desktop remains the first Beta host, but now as a reference implementation for
the cross-platform architecture rather than a special-case endpoint.

Deliverables:

- clarify the desktop shell strategy
- reduce dependence on subprocess-only playback assumptions
- define desktop lifecycle, state restore, and local asset handling in a way
  that does not block mobile
- keep Linux viable where practical, while Apple Silicon remains the main
  validation target

Exit condition:

- the desktop host exercises the same engine boundaries that mobile will need

## Priority 6 — Mobile Feasibility Spikes

Beta must stop calling mobile "future work" and instead produce concrete
answers.

Deliverables:

- iOS host memo: runtime shape, storage shape, audio session shape, Kokoro/ORT
  viability
- Android host memo: runtime shape, storage shape, audio session shape,
  Kokoro/ORT viability
- identify what can stay shared and what must be host-native

Exit condition:

- mobile is no longer conceptual; the blocking technical unknowns are named
  and bounded

## Priority 7 — Re-scope Beta Feature Work

Feature work now follows platform architecture, not the other way around.

Allowed Beta feature work:

- features that harden the shared product model
- features that exercise the engine contract
- features needed by both desktop and mobile hosts

Deferred unless required by the platform plan:

- editor integrations
- cloud sync
- web UI
- broad provider experimentation outside Kokoro
- Windows support

## Immediate Beta Sequence

1. Finalize Beta ADR set for target platforms, Rust engine ownership, runtime
   separation, and TTS direction.
2. Audit the current backend for process-bound and Unix-bound assumptions.
3. Define the Rust engine boundary in concrete DTO and host terms.
4. Write the desktop reference-host plan.
5. Write the iOS and Android feasibility plans.
6. Only then reopen feature delivery against the Beta architecture.

## Open Questions

- Which parts of the current Python backend remain the long-lived reference
  implementation, and which become temporary scaffolding?
- Which local transport should desktop prefer once stdio stops being the only
  serious option?
- How much of playback control belongs to the shared engine versus host-native
  audio control?
- Can command STT stay local and portable across all Beta hosts, or does Beta
  need different host strategies there before feature parity?

## Explicitly Out of Scope for This Reset

- rewriting the full engine immediately
- shipping all three hosts at once
- making Windows a first-class target
- adding cloud-first assumptions
- chasing new TTS providers outside the Kokoro path

## Working Rule

Beta work starts only if it improves the desktop + iOS + Android story.

If a task only improves the current macOS Python host but deepens the gap to
mobile, it is probably not Beta work.
