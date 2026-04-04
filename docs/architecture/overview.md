# Architecture Overview

## Shape

Marginalia is a lightweight modular monolith organized as a monorepo.

- `packages/core` owns domain concepts, events, ports, state machine, and
  application services
- `packages/adapters` owns fake and future concrete providers
- `packages/infra` owns SQLite, config, logging, and event bus wiring
- `apps/cli` owns the first usable interface and composition root

This is effectively a clean or hexagonal architecture with low ceremony.

## Why This Shape

The early product needs strong boundaries more than it needs scale mechanics.
The architecture therefore optimizes for:

- clarity of domain vocabulary
- replaceable infrastructure
- low-cost local iteration
- future reuse by a desktop shell or local API

## Runtime Model

For now, the runtime is simple:

1. the CLI composes a local container
2. application services coordinate domain workflows
3. ports abstract speech, playback, storage, and LLM operations
4. SQLite stores documents, sessions, notes, and draft placeholders
5. fake adapters stand in for real providers

## Why CLI First

CLI-first is not an aesthetic choice. It keeps the first implementation honest:

- session transitions can be defined before UI complexity exists
- provider contracts can be exercised without desktop decisions
- tests can target deterministic commands and outputs
- the same service graph can later back a desktop shell

## Why Monorepo

The product is small enough that repo fragmentation would increase coordination
cost immediately. A monorepo makes cross-cutting changes to docs, CLI, core, and
infra cheap while keeping boundaries explicit in the folder structure.

## Why Python Core

Python is the practical choice for:

- local AI tooling and model integration
- text manipulation
- CLI ergonomics
- future provider ecosystem compatibility

It is also fast enough for the current control-plane work, while the hot path
audio or model details can stay isolated behind adapters later.

## Why SQLite Now

SQLite is sufficient for early persistence requirements:

- single-user local workflow
- low operational overhead
- transactional storage for documents, sessions, and notes
- easy backup and inspection

It is intentionally not treated as a forever constraint, but it is the right
starting point for a local-first product.
