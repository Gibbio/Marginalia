# marginalia-devtools

Development tooling for the Marginalia Beta engine.

The first tool is a small Rust CLI for exercising the runtime without the
Python backend or the TUI.

Current commands:

- `fake-play <document>`
- `sqlite-ingest <db> <document>`
- `sqlite-list-documents <db>`
- `sqlite-play <db> <document>`
- `sqlite-play-target <db> <path|document_id>`
- `sqlite-pause <db>`
- `sqlite-resume <db>`
- `sqlite-stop <db>`
- `sqlite-repeat <db>`
- `sqlite-next-chunk <db>`
- `sqlite-previous-chunk <db>`
- `sqlite-next-chapter <db>`
- `sqlite-previous-chapter <db>`
- `sqlite-restart-chapter <db>`
- `sqlite-note <db> <text>`
- `sqlite-status <db>`
