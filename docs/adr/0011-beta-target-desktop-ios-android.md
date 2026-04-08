# ADR 0011: Beta Targets Desktop, iOS, and Android

- Status: Accepted

## Context

Marginalia Alpha validated the core reading loop as a local Python backend with
desktop-first adapters. That was the correct shape for proving the product.

Beta has a different job. It must stop behaving like a macOS-only experiment
and start behaving like a product family that can be hosted on:

- desktop
- iOS
- Android

The current repository already has useful separation between core logic,
providers, infrastructure, and frontend contracts. It does not yet have a Beta
planning baseline that treats mobile as a first-class target instead of a
future afterthought.

## Decision

Adopt desktop, iOS, and Android as the Beta target platforms.

For Beta planning purposes:

- desktop is not a temporary shell; it is a first-class product host
- iOS is not deferred to a later generation of the product
- Android is not deferred to a later generation of the product
- Windows is explicitly not a Beta priority

Every new architectural and roadmap decision should now be evaluated against
this host set.

## Consequences

- Beta work must optimize for portable runtime and provider choices
- Beta work must avoid desktop-only assumptions in playback, process
  supervision, transport, and device access
- document and state models should remain host-neutral
- host-specific code should move toward thin shells over shared contracts and
  shared domain workflows
- "works on macOS" is no longer enough to count as architectural completion

## Non-Goals

This decision does not require:

- simultaneous feature parity on all three target platforms on day one
- immediate Windows support
- abandoning the current Python desktop implementation before Beta planning is
  complete

It does require that new work stop baking in assumptions that block iOS or
Android later.

## Alternatives Considered

- keep Beta desktop-only and revisit mobile after release: rejected because it
  would defer the hard architectural decisions and make later mobile work more
  expensive
- target every desktop platform equally in Beta: rejected because Linux and
  Apple Silicon matter more than Windows for the current product direction
