# Marginalia

Marginalia is a local AI-first voice reading and annotation engine.

The repository now contains two layers of truth at once:

- the current Alpha reference implementation in Python
- the Beta migration toward a shared Rust engine with desktop, iOS, and
  Android hosts

If you read only one thing before making changes, read
[`NEXT.md`](/home/debian/sources/Marginalia/NEXT.md).

## Current State

What exists today:

- the Alpha local loop is still runnable from the Python codebase
- the Rust TUI in [`apps/tui-rs`](/home/debian/sources/Marginalia/apps/tui-rs)
  is retained as a desktop development and administration tool
- the Beta repository structure is now in place under
  [`crates/`](/home/debian/sources/Marginalia/crates/)
  and [`models/`](/home/debian/sources/Marginalia/models/)
- the first Beta engine crate is
  [`marginalia-core`](/home/debian/sources/Marginalia/crates/marginalia-core)

What is no longer the target architecture:

- a Python child-process backend as the durable product runtime
- stdio as the assumed permanent app boundary
- a desktop-only host model

## Beta Direction

Beta is being shaped around:

- one shared Rust engine
- host applications for desktop, iOS, and Android
- SQLite as local persistence
- Kokoro as the canonical local TTS model family
- ONNX Runtime as the target inference runtime where possible
- explicit contracts between the shared engine and host shells

The important architectural documents are:

- [`NEXT.md`](/home/debian/sources/Marginalia/NEXT.md)
- [`docs/adr/0011-beta-target-desktop-ios-android.md`](/home/debian/sources/Marginalia/docs/adr/0011-beta-target-desktop-ios-android.md)
- [`docs/adr/0013-adopt-rust-engine-with-native-host-shells.md`](/home/debian/sources/Marginalia/docs/adr/0013-adopt-rust-engine-with-native-host-shells.md)
- [`docs/architecture/beta-repository-structure.md`](/home/debian/sources/Marginalia/docs/architecture/beta-repository-structure.md)
- [`docs/migration/alpha-to-beta-repo-mapping.md`](/home/debian/sources/Marginalia/docs/migration/alpha-to-beta-repo-mapping.md)

## Repository Map

- [`packages/`](/home/debian/sources/Marginalia/packages/) holds the Alpha
  Python implementation and remains the migration reference
- [`crates/`](/home/debian/sources/Marginalia/crates/) holds the shared Rust
  Beta engine crates
- [`apps/`](/home/debian/sources/Marginalia/apps/) holds host applications and
  tools
- [`models/`](/home/debian/sources/Marginalia/models/) holds local model asset
  layout
- [`docs/`](/home/debian/sources/Marginalia/docs/) holds ADRs, architecture,
  product notes, and migration records

## Working In This Repo

Run the full Rust test suite:

```bash
cargo test
```

Run the Rust TUI (Beta desktop path, no Python required):

```bash
make tui-rs
```

To enable Kokoro TTS, point `KOKORO_ASSETS_DIR` at a directory
containing the model, config, voices, and ONNX Runtime library.
See [`models/tts/kokoro/README.md`](models/tts/kokoro/README.md)
for the expected layout and download instructions.

```bash
make tui-rs KOKORO_ASSETS_DIR=.kokoro-assets
```

Verify that Kokoro assets are correctly installed:

```bash
make beta-doctor KOKORO_ASSETS_DIR=.kokoro-assets
```

For the Alpha Python environment (migration reference only):

```bash
make bootstrap
make setup
```

More setup detail lives in
[`docs/contributing/development-setup.md`](docs/contributing/development-setup.md).

## Documentation Notes

Some architecture and testing documents still describe the Alpha Python host in
detail. When they disagree with the Beta ADRs and migration docs, treat them as
historical reference unless they have already been updated explicitly for Beta.
