# Changelog

All notable changes to this project will be documented in this file.

The format is inspired by Keep a Changelog and this project aims to follow
semantic versioning once public releases begin.

## [Unreleased]

### Added

- interactive `shell` command: a `cmd.Cmd`-based REPL with play, pause,
  resume, stop, repeat, rewind, next, restart, status, documents, notes,
  ingest, note, and doctor commands — background `RuntimeLoop` thread
  keeps playback running while accepting new commands
- 8 new shell tests covering quit, exit, unknown commands, status,
  pause delegation, doctor, ingest validation
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
- PID reuse protection: runtime supervisor records OS process start time and
  skips termination when start time mismatches (prevents killing unrelated
  processes after PID recycling)
- advisory file locking (`fcntl.flock`) on the runtime JSON to prevent
  concurrent CLI invocations from racing on activate/cleanup/clear
- session auto-expiry: stale `is_active=1` sessions are deactivated on
  startup when they exceed `session_max_inactive_hours` (default 24 h,
  configurable via `MARGINALIA_SESSION_MAX_INACTIVE_HOURS`)
- `_handled_commands` list capped at 1000 entries in `RuntimeLoop` to
  prevent unbounded memory growth during very long reading sessions
- 5 new tests: session expiry (stale + recent), PID reuse protection,
  file locking non-deadlock, command cap enforcement
- sentence-aware chunking: long paragraphs are split at punctuation
  boundaries, short consecutive paragraphs are merged into reading-sized
  units (configurable via `chunk_target_chars`, default 300)
- 10 new chunking tests covering merge, split, mixed content, offsets,
  edge cases, and the real voice-test document
- reading progress tracking: `status`, `synchronize_active_session`, and
  voice-status responses now include a `progress` dict with
  `section_index/section_count`, `chunk_index/section_chunk_count`, and
  `chunks_read/total_chunks` fractions
- `READING_PROGRESSED` event payload enriched with `section_count`,
  `section_chunk_count`, `chunks_read`, and `total_chunks` totals
- chapter boundary logging in `advance_after_playback_completion()`
- `REWIND` voice command intent: go back one chunk (or to the last chunk
  of the previous section) — Italian phrases `indietro`, `precedente`;
  English phrases `back`, `previous`
- 10 new tests covering progress fractions in status/sync/events and
  rewind behavior (within section, cross-section, document start, voice
  dispatch)
- `WhisperCppDictationTranscriber` real adapter: records from the
  microphone, invokes the whisper.cpp binary, parses output into a
  `DictationTranscript` — enables real note dictation on Apple Silicon
- whisper.cpp config: `[whisper_cpp]` TOML section with `executable`,
  `model_path`, `language`, `max_record_seconds`; env overrides
  `MARGINALIA_WHISPER_CPP_*`
- `doctor` reports whisper-cpp readiness (executable, model, sounddevice)
- bootstrap wires `dictation_stt = "whisper-cpp"` with the standard
  fallback pattern
- `make bootstrap-whisper` clones, builds whisper.cpp, and downloads the
  base GGML model
- 9 new whisper tests: capabilities, error paths, doctor section,
  bootstrap fallback/selection, default settings
- background pre-synthesis: after starting playback of chunk N, the next
  chunk's audio is synthesized in a daemon thread — when chunk N finishes,
  the cached WAV is already on disk, eliminating the inter-chunk TTS gap
- 4 new pre-synthesis tests: trigger, last-chunk no-op, cache reuse,
  daemon thread

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
