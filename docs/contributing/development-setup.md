# Development Setup

## Two Working Tracks

Marginalia currently has two active development tracks:

- Beta engine work in Rust
- Alpha reference runtime work in Python

Pick the setup that matches the part of the repository you are changing.

## Beta Engine Setup

Use this when working under:

- `crates/`
- `models/`
- Beta ADRs and migration docs

Baseline prerequisites:

- Rust toolchain
- `cargo`

Current bootstrap:

```bash
cargo test -p marginalia-core
```

At the moment the Rust workspace is intentionally small. The first shared crate
is `marginalia-core`, and more Beta crates will be added incrementally.

Useful Beta documents:

- [`NEXT.md`](/home/debian/sources/Marginalia/NEXT.md)
- [`docs/architecture/beta-repository-structure.md`](/home/debian/sources/Marginalia/docs/architecture/beta-repository-structure.md)
- [`docs/migration/alpha-to-beta-repo-mapping.md`](/home/debian/sources/Marginalia/docs/migration/alpha-to-beta-repo-mapping.md)

## Alpha Reference Setup

Use this when working under:

- `packages/`
- `apps/backend`
- `apps/cli`
- Alpha runtime verification docs

Recommended prerequisites:

- Python 3.12+
- `make`

Development-only setup with fake providers:

```bash
make bootstrap
```

Full Alpha setup with current real providers:

```bash
make setup
```

That path still bootstraps the current Python local loop, including Kokoro,
Vosk, whisper.cpp, config generation, and `doctor`.

## Common Commands

Beta engine:

```bash
cargo test -p marginalia-core
```

Alpha Python reference:

```bash
make test
make smoke
make doctor
make tui-rs
```

## Configuration

For Beta engine work, there is not yet a host-level runtime configuration story
to set up locally.

For Alpha reference work, the main config remains `marginalia.toml`. The
quickest way to validate it is:

```bash
.venv/bin/python -m marginalia_cli doctor --json
```

The example Alpha config remains:

```bash
examples/alpha-local-config.toml
```

## Verification Guidance

Use the Beta path when you are changing shared engine code or migration
documents.

Use the Alpha path when you are validating current runtime behavior, providers,
or the still-runnable desktop reference loop.

The detailed Alpha runtime verification flow remains in
[`docs/testing/alpha-0.1-runtime-loop.md`](../testing/alpha-0.1-runtime-loop.md)
and should be treated as Alpha-specific reference material.
