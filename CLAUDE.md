# CLAUDE.md

The local source of truth for this repository is `AGENTS.md`.

Before editing:

1. read `AGENTS.md` in this repository
2. summarize the requested change
3. identify affected areas (this repo and any consumer surface)
4. read relevant docs (`README.md`, the crate / app README)
5. produce an impact analysis covering TUI / CLI / mac-gui / port
   traits / `marginalia.toml` schema / FFI / docs / tests / scope
6. only then modify code

If the maintainer has provided additional project context for the
session, follow it. In doubt, stop and ask.
