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

## Interaction

Command bar:

- `Tab` completes the selected suggestion
- `Enter` confirms the selected suggestion, then runs the command once the
  input is complete
- `Up` and `Down` navigate suggestions while typing
- `Ctrl-P` and `Ctrl-N` navigate command history
- `Ctrl-C` must be pressed twice within 2 seconds to quit

Session navigation:

- with an empty command bar, `Up` triggers `previous_chunk`
- with an empty command bar, `Down` triggers `next_chunk`
- with an empty command bar, `Left` triggers `previous_chapter`
- with an empty command bar, `Right` triggers `next_chapter`

`/ingest` accepts markdown and plain text files. In the current TUI it also:

- expands shell-like paths such as `~/notes/book.md` or `$HOME/notes/book.txt`
- suggests `.md`, `.markdown`, and `.txt` files from the directory you are
  currently typing
- updates the `Document` pane immediately after a successful import
