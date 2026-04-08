# ADR 0013: Adopt a Rust Engine with Native Host Shells

- Status: Accepted

## Context

Marginalia Beta targets desktop, iOS, and Android.

The Alpha repository proved the product shape with a Python backend, but that
host model is not a credible long-term runtime story for all Beta targets.

Beta needs:

- one shared engine
- one shared product model
- strong portability across desktop, iOS, and Android
- predictable performance for local AI, audio, and persistence work
- host-specific freedom for UI, audio session behavior, and lifecycle control

Kotlin Multiplatform remains a plausible option, but the project wants a
technology base that is especially strong for:

- local AI/runtime integration
- storage and systems-level reliability
- explicit ownership boundaries
- native host embedding

## Decision

Adopt Rust as the shared Marginalia Beta engine language.

Adopt native or host-specific shells on top of that shared engine:

- desktop host application
- iOS host application
- Android host application

Adopt FFI bindings as the boundary between the Rust engine and host shells.

This means:

- shared engine logic moves into Rust crates
- host UI and OS integration stay outside the engine
- the current Python implementation remains a migration reference, not the Beta
  target architecture

## Consequences

- domain, application, contracts, persistence, and provider boundaries become
  Rust-owned over time
- iOS and Android do not need to host a Python child-process backend
- host-specific concerns such as permissions, audio session control, and app
  lifecycle remain in host shells
- migration work becomes explicit and staged rather than accidental

## Non-Goals

This decision does not require:

- immediate deletion of the Alpha Python implementation
- shipping all hosts simultaneously
- one shared UI framework across desktop, iOS, and Android

It does require that new Beta engine work stop assuming Python as the durable
runtime center of the product.

## Alternatives Considered

- keep Python as the long-term shared engine: rejected because it does not fit
  the intended Beta host model well enough
- Kotlin Multiplatform as the shared engine: rejected for now because Rust is a
  stronger fit for the product's systems and local-runtime constraints
- separate native implementations per host: rejected because it would fragment
  the product model and slow feature evolution
