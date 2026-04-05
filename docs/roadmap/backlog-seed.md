# Backlog Seed

## Foundation And V0 Completed

These slices are now present in the repository and should be treated as the
current baseline, not as open bootstrap work:

- repository foundation and CI
- core domain skeleton
- reading session state model
- standardized in-process event model
- SQLite schema v1 foundation
- normalized document storage for sections and chunks
- runnable CLI command surface
- deterministic fake providers behind ports
- provider capability model and diagnostics
- configuration and logging setup
- unit tests, integration tests, and smoke flow
- devcontainer and local development tooling
- real local Piper TTS adapter behind the speech port
- real local Vosk command recognizer behind the command-STT port
- local subprocess playback adapter
- bounded CLI voice control loop with deterministic fake smoke coverage

## Next: Introduce explicit SQLite migrations

Purpose: move from schema bootstrap to deliberate schema evolution.

Suggested labels: `type:feature`, `area:storage`, `area:infra`, `size:m`

Context: `office`

Acceptance criteria:

- migration files or a lightweight migration runner exist
- schema version updates are deliberate
- docs explain how migrations run locally and in CI

## Next: Add document inspection commands

Purpose: let users inspect local documents, notes, and drafts without opening
SQLite manually.

Suggested labels: `type:feature`, `area:cli`, `area:storage`, `size:s`

Context: `office`

Acceptance criteria:

- CLI can list stored documents
- CLI can inspect notes and rewrite drafts for a document
- output remains structured and script-friendly

## Next: Improve chunking and reading progress semantics

Purpose: make repeat, restart, and future progress tracking more realistic.

Suggested labels: `type:feature`, `area:core`, `area:cli`, `size:m`

Context: `home`

Acceptance criteria:

- chunking strategy is more deliberate than paragraph-only splitting
- progress-related events remain stable
- repeat output reflects a more precise reading unit

## Next: Harden real provider packaging and setup

Purpose: reduce friction for macOS local alpha runs with Piper and Vosk.

Suggested labels: `type:feature`, `area:voice`, `area:infra`, `size:m`

Context: `office`

Acceptance criteria:

- setup docs clearly cover provider installation pitfalls
- `doctor` offers clearer remediation hints for missing real providers
- local config examples stay accurate as provider requirements evolve

## Next: Strengthen event payload contracts

Purpose: prepare for future observability or background processing without
changing event names again.

Suggested labels: `type:feature`, `area:core`, `area:infra`, `size:s`

Context: `home`

Acceptance criteria:

- event payload fields are documented in one place
- service tests cover critical emitted events
- no event names are ambiguous about lifecycle intent

## Next: Add draft review workflow

Purpose: make rewrite output more actionable than a single generated blob.

Suggested labels: `type:feature`, `area:llm`, `area:cli`, `size:m`

Context: `home`

Acceptance criteria:

- drafts can be listed and retrieved
- draft status transitions are explicit
- docs explain what remains fake versus real

## Next: Improve doctor diagnostics

Purpose: make local environment failures easier to diagnose.

Suggested labels: `type:feature`, `area:infra`, `area:cli`, `size:s`

Context: `office`

Acceptance criteria:

- doctor checks config readability
- doctor reports writable path issues clearly
- doctor output stays useful in JSON mode
- doctor reports whether real provider requests resolved to fake fallbacks

## Next: Add summary persistence or review design

Purpose: clarify whether summaries should remain transient or become stored
artifacts.

Suggested labels: `type:research`, `area:llm`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- summary request and result lifecycle is described
- persistence expectations are explicit
- provider contract changes are documented if needed

## Next: Expand CLI flow coverage

Purpose: keep the V0 skeleton trustworthy as commands evolve.

Suggested labels: `type:feature`, `area:ci`, `area:cli`, `size:s`

Context: `office`

Acceptance criteria:

- tests cover play-without-document-id using latest document fallback
- tests cover planned and error paths for rewrite and note capture
- smoke coverage remains deterministic
- tests cover real-provider fallback and voice command routing

## Next: Improve command recognition robustness

Purpose: make the local command loop less brittle without broadening scope into
full dictation.

Suggested labels: `type:feature`, `area:voice`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- command normalization supports a few safe spoken variants
- Vosk grammar remains intentionally small
- docs stay explicit about what commands are actually supported

## Next: Evolve note search beyond substring matching

Purpose: determine whether note search should remain SQLite substring search or
grow into indexing later.

Suggested labels: `type:research`, `area:storage`, `area:core`, `size:m`

Context: `home`

Acceptance criteria:

- ranking expectations are described
- schema and query tradeoffs are captured
- future migration options are documented

## Next: Spike future editor adapter boundary

Purpose: prepare for eventual editor integration without contaminating the core.

Suggested labels: `type:research`, `area:future-editor`, `size:m`

Context: `home`

Acceptance criteria:

- the spike produces a short decision memo
- no core package depends on editor APIs
- adapter boundaries are proposed rather than merged into the core
