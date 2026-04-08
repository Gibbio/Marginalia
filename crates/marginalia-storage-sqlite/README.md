# marginalia-storage-sqlite

SQLite persistence for the Marginalia Beta engine.

The first version provides:

- schema migrations
- SQLite-backed document, session, note, and rewrite repositories
- in-memory and file-backed database opening

It mirrors the Alpha schema direction closely enough to keep migration simple.
