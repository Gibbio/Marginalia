# Repository Structure

## Intent

The repository is laid out to make domain boundaries and future product surfaces
obvious from the start.

## Top-Level Map

### `apps/`

- `backend/`: headless local backend process
- `cli/`: thin Python compatibility interface and admin/debug surface
- `desktop/`: reserved place for a future thin desktop shell
- `tui-rs/`: Rust TUI client over the backend contract

### `packages/`

- `core/`: domain model, state machine, application services, events, ports
- `adapters/`: fake and future provider integrations
- `infra/`: SQLite, config, logging, and composition-support infrastructure

### `docs/`

- `vision/`: product framing
- `product/`: user flows and scope
- `architecture/`: system-level documentation
- `adr/`: architecture decision records
- `roadmap/`: milestones and backlog seed
- `contributing/`: setup and workflow guidance

### `tests/`

- `unit/`: narrow component tests
- `integration/`: command and service workflow tests
- `fixtures/`: reusable local artifacts

### `scripts/`

Local helper scripts for smoke or bootstrap actions.

### `examples/`

Example documents and future demo artifacts.

## Why Not Separate Repositories

The product is still defining its core abstractions. Splitting repositories now
would make every cross-cutting change slower:

- CLI work depends on core service contracts
- provider adapter work depends on ports
- docs and ADRs should evolve with code

The monorepo keeps those changes adjacent while still preserving boundaries.
