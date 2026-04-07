# Marginalia Rust TUI

This is the Rust frontend for Marginalia.

It talks to the Python backend over `stdio` using the frontend contract exposed
by `marginalia_backend serve-stdio`.

## Run

From the repository root:

```bash
cargo run --manifest-path apps/tui-rs/Cargo.toml
```

If needed, point it at a specific Python interpreter or repo root:

```bash
MARGINALIA_BACKEND_PYTHON=/path/to/.venv/bin/python \
MARGINALIA_REPO_ROOT=/path/to/Marginalia \
cargo run --manifest-path apps/tui-rs/Cargo.toml
```

If `marginalia.toml` exists, export it before launch:

```bash
MARGINALIA_CONFIG=marginalia.toml \
cargo run --manifest-path apps/tui-rs/Cargo.toml
```
