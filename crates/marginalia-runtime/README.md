# marginalia-runtime

Host-neutral runtime composition for the Marginalia Beta engine.

The first implementation in this crate is intentionally small:

- assembles `marginalia-core`
- uses `marginalia-import-text`
- uses `marginalia-provider-fake`
- exposes a fake bootstrap runtime for integration tests and early hosts
- exposes a SQLite-backed runtime profile for Beta desktop tooling

Current runtime entry points:

- `FakeRuntime`: in-memory repositories plus fake providers
- `SqliteRuntime`: SQLite repositories plus fake providers

`apps/tui-rs` can now run directly against `SqliteRuntime` by setting
`MARGINALIA_TUI_BACKEND=beta`.

`SqliteRuntime` also exposes the first embedded frontend gateway for Beta:

- command handling for ingest, playback navigation, and note creation
- query handling for snapshots, document views, doctor report, capabilities,
  note listing, and basic search

This moves the frontend contract out of the legacy Python backend path and into
the shared Rust runtime.
