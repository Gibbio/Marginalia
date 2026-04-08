# Python Removal Plan

This note defines the order for removing Python from the Beta path without
breaking the product during migration.

The key distinction is:

- "Python is not required for Beta runtime"
- "Python still exists in the repository as Alpha compatibility"
- "Python can be deleted from the repository"

Those are three different milestones.

## Phase 1: Beta Runtime No Longer Requires Python

Goal:

- desktop Beta can run and be developed without Python installed

This phase is complete only when:

- `apps/tui-rs` runs in normal Beta mode with no Python dependency
- the Beta build path does not require `marginalia_backend`
- the Beta build path does not require `marginalia_cli`
- the Beta build path does not require `packages/core`, `packages/adapters`, or
  `packages/infra`
- the Beta runtime owns the frontend gateway used by the TUI

Current status:

- mostly complete
- `apps/tui-rs` defaults to the Rust runtime
- Alpha compatibility is isolated behind `alpha-compat`

Remaining work in Phase 1:

- decide whether to keep or delete `alpha-compat`
- make sure desktop smoke flows are documented and tested entirely through Rust
- stop relying on Alpha docs for normal local development

Exit condition:

- a new contributor can build and run the Beta desktop toolchain without Python

## Phase 2: Replace Python Reference Components

Goal:

- the Beta product has Rust-owned equivalents for the pieces that still only
  exist in Python

Required replacements:

- real playback provider for desktop hosts
- real TTS path in Rust, targeting Kokoro + ONNX Runtime
- real STT strategy for Beta, or an explicit temporary product decision to ship
  without it
- Rust CLI or devtool coverage for the Alpha CLI flows that still matter
- Rust-owned config, logging, and runtime wiring for Beta hosts

Python areas still acting as reference implementations today:

- `apps/backend`
- `apps/cli`
- `packages/core`
- `packages/adapters`
- `packages/infra`

The biggest blockers are the real providers:

- playback
- Kokoro inference/runtime integration
- command STT / dictation STT

Exit condition:

- no Beta-critical user flow depends on consulting or running the Python stack

## Phase 3: Delete Alpha Python From the Product Path

Goal:

- remove Python code from the active product repository, or archive it outside
  the Beta path

Deletion candidates:

- `apps/backend/src/marginalia_backend/`
- `apps/cli/src/marginalia_cli/`
- `packages/core/src/marginalia_core/`
- `packages/adapters/src/marginalia_adapters/`
- `packages/infra/src/marginalia_infra/`

Before deletion:

- remove `alpha-compat` from `apps/tui-rs`
- remove Python setup instructions from main development docs
- freeze or archive Alpha operational docs
- confirm that Beta tests and smoke flows cover the supported desktop path

Exit condition:

- Python is no longer part of the supported build, runtime, or development
  story for Marginalia Beta

## Recommended Order

1. Finish Phase 1 and explicitly stop treating Python as part of the normal
   Beta path.
2. Build the missing real Rust providers needed for desktop Beta.
3. Replace the remaining Alpha CLI and runtime utilities that still matter.
4. Remove `alpha-compat`.
5. Archive or delete the Python tree.

## Non-Goals

This plan does not mean:

- delete Alpha immediately
- port every historical experiment before moving on
- keep feature parity with Alpha before cleaning up architecture

The correct sequence is:

- remove Python from the active Beta path first
- remove Python from the repository only after the supported Beta path is real
