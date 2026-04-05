# Changelog

All notable changes to this project will be documented in this file.

The format is inspired by Keep a Changelog and this project aims to follow
semantic versioning once public releases begin.

## [Unreleased]

### Added

- `HELP` voice command intent with Italian (`aiuto`, `comandi`) and English
  (`help`, `commands`) phrases
- stop aliases: `fermati` (Italian) and `halt` (English)
- `READING_COMPLETED` domain event emitted when the entire document finishes
  playing, distinguishable from `PLAYBACK_STOPPED` on explicit stop
- `COMMAND_DISPATCHED` domain event emitted after every voice intent dispatch
- structured logging in: `FileRuntimeSupervisor` (cleanup decisions),
  bootstrap (provider selection and fallback), `ReaderService` (command
  dispatch, play, stop, document completion), and `RuntimeLoop` (lifecycle)
- 12 new tests: completion vs stop distinction, help intent dispatch, alias
  resolution (`fermati`, `halt`), restart after completion, status
  truthfulness, provider capability reporting, fallback visibility

### Changed

- refactored voice command dispatch from hardcoded if/elif chain to a
  dict-driven table — adding a new intent requires only an enum member, a TOML
  entry, and a dispatch table entry
- unhandled voice intents now return an explicit error instead of silently
  falling through to stop — this was a real bug where any new intent added to
  the enum but not to the dispatch chain would stop the reading session

## [0.2.0a0] - 2026-04-05

### Added

- step-driven `RuntimeLoop` class that decouples the read-while-listen loop from
  the CLI — the loop can now be driven by a CLI `while` loop, a desktop timer,
  or an async wrapper
- sequential file-based SQLite migration system (`schema_migrations` table,
  numbered `.sql` files under `packages/infra/src/marginalia_infra/storage/migrations/`)
- explicit `is_active` column on sessions replacing implicit
  `ORDER BY updated_at DESC` active-session resolution
- SQLite WAL mode and `busy_timeout = 5000` for concurrent reader/writer safety
- connection caching in `SQLiteDatabase` to avoid repeated open/close overhead
- signal handling (`SIGINT`/`SIGTERM`) in the CLI `play` command for graceful
  shutdown during playback
- audio cache cleanup with configurable `max_age_hours` (default 72 h)
- structured logging with optional file handler (`log_file` setting)
- `ReadingPosition.from_anchor()` classmethod to deduplicate anchor parsing
  across adapters
- Kokoro TTS as the default synthesis provider, Piper retained as optional
- two new unit tests for the step-driven loop (completion and shutdown-request)

### Changed

- `ReadingRuntimeService` is now a thin wrapper around `RuntimeLoop` — desktop
  or async callers use `create_loop()` directly
- CLI `play` command drives the loop externally with signal handlers instead of
  blocking inside the service
- schema version bumped to v4 (`sqlite-v4-migrated`)
- project version bumped to `0.2.0a0`

### Removed

- dead `StorageCoordinator` compatibility alias
- inline `SCHEMA_SQL` constant and `_ensure_column` hack in SQLite infra
- duplicated `_position_from_anchor()` helpers in playback adapters

## [0.1.0a0] - 2026-04-05

### Added

- initial monorepo bootstrap
- architecture documentation and ADR set
- CLI skeleton with SQLite-backed local stubs
- CI, devcontainer, and contribution workflow scaffolding
- document ingestion, session lifecycle, and playback commands
- real local Kokoro/Piper TTS, Vosk command STT, and subprocess playback adapters
- language-specific voice command lexicon system
- note capture, rewrite draft, topic summary, and search services
- `doctor` and `status` CLI diagnostics
- end-to-end smoke flow
