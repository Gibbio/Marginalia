# Backlog Seed

## Repository foundation setup

Purpose: keep the repository bootstrap coherent and repeatable.

Suggested labels: `type:feature`, `area:infra`, `area:docs`, `size:m`

Context: `either`

Acceptance criteria:

- root hygiene files stay current
- local bootstrap flow remains documented
- CI stays aligned with the actual toolchain

## Establish core domain package skeleton

Purpose: stabilize product vocabulary before deeper implementation work.

Suggested labels: `type:feature`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- document, session, note, rewrite, summary, and search models exist
- package boundaries remain clear
- tests cover the main bootstrap-level invariants

## Define reading session state model

Purpose: prevent ad hoc lifecycle behavior as commands grow.

Suggested labels: `type:feature`, `area:core`, `area:cli`, `size:s`

Context: `home`

Acceptance criteria:

- state graph is documented and enforced
- invalid transitions fail explicitly
- CLI commands use the same state vocabulary

## Define event model

Purpose: make future observability and async expansion possible without guessing event names later.

Suggested labels: `type:feature`, `area:core`, `area:infra`, `size:s`

Context: `home`

Acceptance criteria:

- event names are standardized
- payload expectations are documented
- in-process publisher abstraction remains minimal and testable

## Define SQLite schema v0

Purpose: create a stable first storage contract for documents, sessions, notes, and drafts.

Suggested labels: `type:feature`, `area:storage`, `area:infra`, `size:m`

Context: `office`

Acceptance criteria:

- tables are documented
- migration strategy is sketched
- repositories round-trip the core models they claim to support

## Implement CLI scaffolding

Purpose: provide a usable interface for early product learning.

Suggested labels: `type:feature`, `area:cli`, `size:m`

Context: `office`

Acceptance criteria:

- required commands exist with coherent help text
- CLI is thin and delegates to services
- smoke tests cover the supported local paths

## Add fake providers

Purpose: unblock architectural progress without pretending real provider behavior exists.

Suggested labels: `type:feature`, `area:voice`, `area:llm`, `size:s`

Context: `office`

Acceptance criteria:

- fake adapters sit behind ports
- outputs are explicit placeholders
- no fake adapter leaks into the core as a concrete dependency

## Add configuration system

Purpose: standardize local paths and future provider selection.

Suggested labels: `type:feature`, `area:infra`, `size:s`

Context: `office`

Acceptance criteria:

- runtime paths are configurable
- provider selections are explicit
- doctor output reflects the active configuration

## Add logging conventions

Purpose: keep local troubleshooting consistent as the CLI grows.

Suggested labels: `type:feature`, `area:infra`, `size:xs`

Context: `office`

Acceptance criteria:

- logging setup is centralized
- CLI verbose mode is supported
- future providers can plug into the same conventions

## Write unit tests for initial stubs

Purpose: prove the skeleton is intentional rather than decorative.

Suggested labels: `type:feature`, `area:ci`, `area:core`, `size:s`

Context: `office`

Acceptance criteria:

- state machine tests exist
- repository tests exist
- CLI smoke tests exist

## Document architecture

Purpose: keep decisions visible and stable as implementation accelerates.

Suggested labels: `type:docs`, `area:docs`, `type:adr`, `size:m`

Context: `home`

Acceptance criteria:

- overview, state machine, domain model, and repository structure docs are current
- ADR set covers the current hard decisions
- docs explain what is intentionally deferred

## Add devcontainer

Purpose: make it easier to continue work across machines without re-deriving the environment.

Suggested labels: `type:feature`, `area:infra`, `size:s`

Context: `office`

Acceptance criteria:

- devcontainer opens with the expected Python toolchain
- local bootstrap command is part of container setup
- recommended editor extensions remain minimal

## Spike future Obsidian adapter

Purpose: explore editor integration only after the core contract is stable.

Suggested labels: `type:research`, `area:future-editor`, `size:m`

Context: `home`

Acceptance criteria:

- the spike produces a short decision memo
- no core package depends on editor APIs
- adapter boundaries are proposed, not merged into the domain

## Spike topic summarization pipeline

Purpose: clarify how summaries should be requested, cached, and reviewed.

Suggested labels: `type:research`, `area:llm`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- summary request and result lifecycle is described
- provider contract changes are documented if needed
- persistence expectations are explicit

## Spike search-notes design

Purpose: determine whether note search should remain SQLite-only or add indexing later.

Suggested labels: `type:research`, `area:storage`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- ranking expectations are described
- schema and query tradeoffs are captured
- future migration options are documented
