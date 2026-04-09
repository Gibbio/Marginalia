# Marginalia Rust TUI

This is the Rust TUI frontend for Marginalia, built with ratatui.

In the Beta plan it is retained as the desktop development and
administration tool while the shared Rust engine matures. It talks
directly to the embedded `SqliteRuntime` — no Python process is
involved.

## Run

```bash
cargo run --manifest-path apps/tui-rs/Cargo.toml
```

or via Make:

```bash
make tui-rs
```

On startup the TUI shows a loading screen while the runtime
initialises, then fetches the doctor report and surfaces any provider
issues in the `Log` pane.

## Providers

### Playback

`HostPlaybackEngine` is active by default. It auto-detects `afplay`
(macOS), `aplay` (Linux), or `ffplay` and plays synthesised WAV files
via a subprocess.

To disable playback (CI, headless environments):

```bash
MARGINALIA_TUI_PLAYBACK=fake cargo run --manifest-path apps/tui-rs/Cargo.toml
```

### TTS — Kokoro

Kokoro TTS is activated when `MARGINALIA_KOKORO_ASSETS` points at a
directory containing the model, config, voices, and ONNX Runtime
library. See [`models/tts/kokoro/README.md`](../../models/tts/kokoro/README.md)
for the expected layout and download instructions.

```bash
MARGINALIA_KOKORO_ASSETS=.kokoro-assets \
cargo run --manifest-path apps/tui-rs/Cargo.toml
```

If `MARGINALIA_KOKORO_ASSETS` is not set, or the assets directory is
incomplete, the TUI falls back to a silent fake TTS and logs a warning
in the `Log` pane. The session still starts; no audio is produced.

Synthesised WAV files are written to `.marginalia-tts-cache/` next to
the database by default. Override with `MARGINALIA_TUI_TTS_DIR`.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `MARGINALIA_KOKORO_ASSETS` | _(unset = fake TTS)_ | Path to Kokoro model assets directory |
| `MARGINALIA_TUI_TTS_DIR` | `<db_dir>/.marginalia-tts-cache/` | Directory for synthesised WAV files |
| `MARGINALIA_TUI_PLAYBACK` | `host` | Set to `fake` for headless/CI |
| `MARGINALIA_TUI_BETA_DB` | `.marginalia-beta.sqlite3` | SQLite database path |
| `MARGINALIA_REPO_ROOT` | current directory | Repository root for relative paths |
| `MARGINALIA_TUI_LOG_FILE` | `marginalia-tui.log` | Client-side log file path |

## Interaction

Command bar:

- `Tab` completes the selected suggestion
- `Enter` confirms the selected suggestion, then runs the command
- `Up` and `Down` navigate suggestions while typing
- `Ctrl-P` and `Ctrl-N` navigate command history
- `Ctrl-C` must be pressed twice within 2 seconds to quit

Session navigation (with an empty command bar):

- `Up` / `Down` — previous / next chunk
- `Left` / `Right` — previous / next chapter
- `PageUp` / `PageDown` — scroll the Document pane
- `Home` / `End` — jump to top / bottom of the Document pane

`/ingest` accepts markdown and plain text files. It expands
shell-like paths (`~/notes/book.md`, `$HOME/...`) and suggests
`.md`, `.markdown`, and `.txt` files as you type.
