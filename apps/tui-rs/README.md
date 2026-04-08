# Marginalia Rust TUI

This is the Rust TUI frontend for Marginalia.

In the Beta plan it is retained as a desktop development and administration
tool. It is not assumed to be the final consumer desktop shell, but it remains
an important Rust host during the migration away from the Alpha Python-centered
runtime model.

It talks to the Python backend over `stdio` using the frontend contract exposed
by `marginalia_backend serve-stdio`.

During Beta migration, that transport may change as the shared engine boundary
stabilizes. The TUI itself is still expected to survive and evolve.

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

The TUI also appends its own client-side logs to `marginalia-tui.log` in the
current working directory. To choose a different path:

```bash
MARGINALIA_TUI_LOG_FILE=/tmp/marginalia-tui.log \
cargo run --manifest-path apps/tui-rs/Cargo.toml
```

On startup the TUI now shows a lightweight loading screen while the backend and
its configured providers initialize. As soon as the backend is ready, the TUI
fetches the backend doctor report and surfaces missing executables or provider
fallbacks in the `Log` pane.

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
- with an empty command bar, `PageUp` and `PageDown` scroll the `Document` pane
- with an empty command bar, `Home` jumps to the top of the `Document` pane
- with an empty command bar, `End` jumps to the bottom of the `Document` pane

`/ingest` accepts markdown and plain text files. In the current TUI it also:

- expands shell-like paths such as `~/notes/book.md` or `$HOME/notes/book.txt`
- suggests `.md`, `.markdown`, and `.txt` files from the directory you are
  currently typing
- updates the `Document` pane immediately after a successful import

The `Document` pane now renders the full document outline and auto-follows the
active chunk while keeping the backend timing logs visible in the `Log` pane.
