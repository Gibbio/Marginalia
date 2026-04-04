# Contributing

Marginalia is being built as a long-lived product foundation. Contributions
should improve clarity, maintainability, and architectural coherence rather than
maximize short-term feature count.

Before opening a pull request:

1. Read `docs/contributing/development-setup.md`.
2. Read `docs/contributing/workflow.md`.
3. Check whether the change should update an ADR in `docs/adr/`.
4. Run `make lint`, `make test`, and `make smoke`.

Expected contribution characteristics:

- small, coherent changesets
- tests for behavior that is actually implemented
- documentation updates for architecture or workflow changes
- no direct coupling from the core to concrete editors, remote APIs, or UI code

If the change alters repository-level architecture, update the relevant ADR or
add a new one.
