# marginalia-runtime

Host-neutral runtime composition for the Marginalia Beta engine.

The first implementation in this crate is intentionally small:

- assembles `marginalia-core`
- uses `marginalia-import-text`
- uses `marginalia-provider-fake`
- exposes a fake bootstrap runtime for integration tests and early hosts
