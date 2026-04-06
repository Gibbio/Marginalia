# Marginalia — Next Steps

This document describes the path from the current pre-Alpha 0.3 state to a
finished product. The priority is a solid, reliable, pleasant-to-use engine
over a feature-complete product shipped fast. Each step should be small enough
to finish in one or two focused sessions.

Last updated: April 2026.

---

## Principles

- **Engine quality first.** Every step should leave the codebase more
  trustworthy, not just bigger. If a step adds features but makes the
  experience worse, it failed.
- **Small steps, always shippable.** Each milestone produces a working state
  that can be used and tested. No long branches, no half-finished features
  behind flags.
- **Real usage drives priorities.** If something feels broken or annoying
  during actual use, fix it before adding new capabilities.
- **Keep the architecture honest.** New features go through ports. New
  persistence goes through migrations. New commands go through the dispatch
  table. No shortcuts.

---

## Current State (pre-Alpha 0.3)

Working:
- Real read-while-listen loop with Kokoro TTS, Vosk commands, afplay
- Voice commands: pause, resume, repeat, rewind, chapter nav, stop, help
- Document ingestion, session persistence, note anchoring
- Runtime supervision with PID reuse protection and file locking
- Interactive shell (`marginalia shell`) with playback, navigation, status,
  documents, notes, ingest, and doctor — runs the RuntimeLoop in a
  background thread
- Background pre-synthesis eliminates inter-chunk TTS gaps
- Sentence-aware chunking with configurable target size
- Reading progress tracking (section/chunk fractions, overall progress)
- Real whisper.cpp dictation adapter for note capture
- `make setup` bootstraps the full stack in one command (system deps,
  Python venv, runtime packages, Kokoro, Vosk model, whisper.cpp, config)
- 106 tests, clean lint and types

Stubbed:
- Rewrite and summary generation (fake providers)
- Desktop UI, editor adapters

Main rough edges:
- No dedicated inspection commands (`show-document`, `list-drafts`,
  `list-sessions`) — basic `documents` and `notes` exist in the shell
- Doctor output is readiness flags only — no remediation hints or checklist
- No way to manage sessions (delete, resume a previous one)
- Windows support not yet implemented (macOS-only)

---

## Step 1 — CLI Inspection and Document Browser [partial]

Partially completed April 2026.

The interactive shell already provides `documents` (list ingested docs)
and `notes` (list notes for the active session). What remains:

- `show-document <id>` — show document outline (sections and chunk counts)
- `list-drafts <document-id>` — show rewrite drafts for a document
- `list-sessions` — show recent sessions with state, document, last command
- **Document browser:** `open` or `browse [directory]` shell command that
  lists files in a folder (default: current dir or a configured library
  path), lets the user pick by number, and auto-ingests + plays — replaces
  the manual `ingest path/to/file.md` workflow
- Add these as both shell commands and Typer subcommands
- tests for each new command

Size: small. No new domain logic, only query paths and CLI surface.

## Step 2 — Smarter Chunking [done]

Completed April 2026.

- Long paragraphs (>1.5x target) split at sentence boundaries (`.!?…`)
- Short consecutive fragments merged until they approach the target
- `chunk_target_chars` setting (default 300, configurable via env or TOML)
- Threaded through `AppSettings` → `DocumentIngestionService` →
  `build_document_outline`
- 10 new tests covering merge, split, mixed content, char offsets, edge cases
- Existing 65 tests unaffected — section/chunk/anchor model unchanged

## Step 3 — Provider Setup Ergonomics [partial]

Partially completed April 2026.

`make setup` now bootstraps the entire stack in one command (system deps,
venv, runtime packages, all providers, config generation, doctor
verification). What remains is improving the `doctor` output:

- `doctor` should print actionable remediation hints, not just readiness flags
  (e.g. "run `make bootstrap-kokoro` to set up Kokoro TTS")
- `doctor` should warn when `allow_fallback = true` and real providers are not
  ready (currently silent)
- Add a setup checklist to `doctor` output: what is ready and what is
  missing, ordered by setup priority
- Update `examples/alpha-local-config.toml` comments to explain each setting

Size: small. Config and diagnostics only, no core changes.

## Step 4 — Reading Progress [done]

Completed April 2026.

- `status`, `synchronize_active_session`, and voice-status responses include
  a `progress` dict: `section_index/section_count`,
  `chunk_index/section_chunk_count`, `chunks_read/total_chunks`
- `READING_PROGRESSED` event enriched with `section_count`,
  `section_chunk_count`, `chunks_read`, `total_chunks`
- Chapter boundary logging in `advance_after_playback_completion()`
- 5 new progress tests
- Estimated time remaining deferred to a later step (requires playback
  timing instrumentation)

## Step 5 — Playback Quality of Life [done]

Completed April 2026.

- `REWIND` voice command intent: go back one chunk, crossing section
  boundaries when needed — Italian `indietro`/`precedente`, English
  `back`/`previous`
- `previous_chunk()` method in `ReaderService`, wired into the dispatch table
- background pre-synthesis: after starting chunk N, chunk N+1 is
  synthesized in a daemon thread — eliminates the inter-chunk TTS gap
  (Kokoro file cache makes the next `synthesize()` call instant)
- 5 rewind tests + 4 pre-synthesis tests

Deferred to a later step:
- `speed` voice command to cycle playback speeds
- configurable inter-chunk pause duration
- context re-read on resume

## Step 6 — Session Management

Let the user manage sessions explicitly.

- `list-sessions` — show recent sessions with state, document, progress
  (prerequisite from Step 1)
- `delete-session <id>` — deactivate and remove a session
- `resume-session <id>` — switch the active session to a previous one
- When starting `play` with no target and no active session, show a list of
  recent sessions the user can resume instead of just picking the latest
  document
- Add as both shell commands and Typer subcommands

Size: small. Query and lifecycle commands, no new domain concepts.

## Step 7 — Real Note Dictation [partial]

Completed April 2026 (whisper.cpp adapter). `make setup` now handles
the full bootstrap including whisper.cpp build and model download.

- `WhisperCppDictationTranscriber` adapter: records from mic via
  `sounddevice`, invokes whisper.cpp `main` binary, parses output
- `[whisper_cpp]` config section with `executable`, `model_path`,
  `language`, `max_record_seconds`
- `doctor` reports whisper-cpp readiness
- bootstrap wires `dictation_stt = "whisper-cpp"` with fallback
- `make bootstrap-whisper` builds whisper.cpp and downloads the base model
- 9 new tests

Remaining:
- **Audio cues for note recording:** play a short tone (beep/chime) when
  note-start begins recording and when note-stop ends it — the user needs
  clear feedback that the mic is live. Use a bundled WAV file played through
  the existing playback engine.
- **Strip the stop command from transcription:** when the user says
  "nota stop" or "note stop" to end dictation, the STT will capture that
  phrase — detect and strip it from the transcript before saving.
  Implementation depends on the STT provider: whisper.cpp returns full text,
  so a simple suffix-strip against the command lexicon should work.
- Raw audio path storage for later review
- Smoke-test with real hardware

## Step 8 — Note Review and Post-Note Workflow

Make captured notes actionable and give the user a choice after dictation.

- **Post-note prompt:** after note-stop, show the transcript and ask the
  user what to do:
  - `keep` — save as a standalone anchored note (current behavior)
  - `rewrite` — pass the note + current section to the rewrite service and
    generate a rework draft that incorporates the note into the text
  - `discard` — throw away the transcript
  - In the shell this is an interactive prompt; via CLI flags (`--action
    keep|rewrite|discard`) for scripted use
- `show-note <id>` — display full note transcript with its anchor position
- `edit-note <id> --transcript "..."` — correct a dictation mistake
- `delete-note <id>` — remove a note
- `play-note <id>` — play back the raw audio if it was saved
- Notes should display their position context: which section title and chunk
  excerpt they are anchored to

Size: small-medium. CLI surface, repository queries, and post-note dispatch.

## Step 9 — Document Format Support

Expand beyond plain text and markdown.

- Add EPUB ingestion (extract text from XHTML content documents, map chapters
  to sections) — use the standard library `zipfile` plus a lightweight HTML
  parser, no heavy dependencies
- Add PDF text extraction as a second format — evaluate `pymupdf` or
  `pdfplumber`, pick whichever has lighter dependencies
- The ingestion port and document model stay the same — format support is a
  parser concern
- `doctor` should report which format parsers are available

Size: medium. New parsers behind the existing ingestion service.

## Step 10 — Real Rewrite Generation

Replace the fake rewrite provider with a real local or hybrid LLM.

- Evaluate `llama-cpp-python` for local inference or a simple OpenAI-compatible
  API client for hybrid use
- The rewrite port (`RewriteGenerator`) already exists — this is an adapter
- The rewrite instruction already carries source text, note transcripts, and
  section context
- Add a `llm` section to config for model path or API endpoint
- Keep the fake provider as the default — real LLM is opt-in
- `doctor` should report LLM provider readiness

Size: medium. New adapter, model/API setup, but port and service exist.

## Step 11 — Draft Review Workflow

Make rewrite drafts more than a generated blob.

- `list-drafts` already added in Step 1 — extend it here
- `show-draft <id>` — display the full draft with source context
- `accept-draft <id>` — mark as accepted (status transition)
- `reject-draft <id>` — mark as rejected
- `regenerate-draft <id>` — request a new generation for the same source
- Draft status transitions: `generated -> accepted | rejected`
- The `RewriteStatus` enum already has `GENERATED` — add `ACCEPTED` and
  `REJECTED`

Size: small. Status enum, CLI commands, repository queries.

## Step 12 — Real Summarization and Voice Search

Replace the fake summarization provider with a real one and add
voice-driven document discovery.

- Reuse the same LLM backend from Step 10
- The summary port (`TopicSummarizer`) already exists
- Summary results should be persisted (currently transient) — add a `summaries`
  table via migration
- `list-summaries <document-id>` and `show-summary <id>`
- `summarize` shell command — summarize the current document or a specific
  section using the real LLM provider
- **Voice search by topic:** `search-topic` voice command or shell command
  that records a short dictation (via the existing `DictationTranscriber`),
  transcribes it, and runs a search across all documents and notes for
  matching content — "find me the document about X" without typing
- Voice search reuses the existing `SearchService` — the new part is
  capturing the query by voice instead of keyboard

Size: medium. New migration, adapter reuse, voice-to-search pipeline.

## Step 13 — Event Persistence

Make domain events queryable after the process exits.

- Add an `events` table via migration
- `InMemoryEventBus` gains an optional SQLite subscriber that writes events
- `list-events --document <id>` — show event history for a document
- `list-events --session <id>` — show event history for a session
- Events become useful for understanding what happened in a past session

Size: small-medium. New migration, subscriber adapter, CLI query.

## Step 14 — Export and Interoperability

Let Marginalia output be useful outside Marginalia.

- `export-notes <document-id> --format markdown` — export all notes as a
  markdown file with section headings and position context
- `export-draft <id> --format markdown` — export a rewrite draft
- `export-session <id> --format markdown` — export a session summary with
  notes, drafts, and reading progress
- These are read-only exports, not sync — keep it simple

Size: small. Formatting and file output, no new domain logic.

## Step 15 — Rich Terminal UI (Textual)

Upgrade the `cmd.Cmd` shell to a Textual TUI for a richer reading
experience.

The `cmd.Cmd`-based shell already provides a functional interactive
experience. This step upgrades it to a full terminal UI:

- **Document text pane:** show the full document text on screen. As
  playback progresses, highlight the current chunk in a distinct color
  (e.g. inverse or bold) and dim already-read chunks — the user always
  sees where they are in the text
- **Status bar (bottom):** persistent footer showing CPU usage, memory,
  current chunk/section progress, playback state, provider info — similar
  to Claude Code's status line ("Now using extra usage"). Updated in
  real-time via the event bus
- **Command input pane:** text input at the bottom for shell commands,
  coexisting with the status bar
- **Note recording indicator:** visual indicator (e.g. blinking "REC" in
  the status bar) when note dictation is active
- Evaluate Textual as the framework — it supports split panes, reactive
  updates, and runs in any terminal
- The `RuntimeLoop.step()` model already supports timer-driven callers
- Voice commands continue through the microphone — the TUI adds visual
  feedback, not new input methods
- Keep it in `apps/cli/` as an upgrade to the existing shell

Size: medium. Upgrade path from existing shell, core and services reused.

## Step 16 — Concurrent Playback and Dictation

Handle the hardest audio challenge.

- When the user says `note-start` during playback, duck or pause playback
  while dictation is active
- Resume playback after `note-stop`
- This requires coordinating the playback engine and dictation transcriber
  within the runtime loop
- Test with Bluetooth headsets (the known hard case)

Size: medium. Runtime loop coordination, audio device management.

## Step 17 — Multi-Document and Cross-Document Features

Expand beyond single-document workflows.

- `search` across all documents and notes (already partially works)
- Cross-document topic summaries
- Document collections or projects as a lightweight grouping mechanism
- Note migration between documents

Size: medium. New domain concepts (collections), service extensions.

## Step 18 — Editor Adapter Spike

Evaluate editor integration without contaminating the core.

- Define export contracts for notes and drafts
- Build a minimal Obsidian vault adapter: export notes as markdown files in a
  vault folder, one file per document with anchored notes
- The adapter consumes exported data — it does not reach into the core
- Produce a short decision memo on whether deeper integration is worth pursuing

Size: small-medium. Adapter code, no core changes.

---

## What Is Explicitly Deferred

These are conscious non-goals until the engine is mature:

- **Cloud sync or multi-user support** — Marginalia is local-first, period
- **Mobile apps** — the audio pipeline assumes macOS for now
- **Real-time collaboration** — single user, single machine
- **Plugin system** — the port architecture is the extension mechanism
- **Web UI** — terminal or desktop only
- **Streaming TTS** — chunk-based synthesis is fine for the current playback
  model; streaming adds complexity without clear UX benefit yet
- **Windows support** — feasible (~half day, mostly infra layer: fcntl,
  signal, process supervision, build scripts) but deferred until the
  engine is mature on macOS

---

## How To Use This Document

1. Pick the next uncompleted step
2. Read the acceptance criteria
3. Implement, test, commit
4. Update this document: mark the step as done, note the date, adjust later
   steps if priorities shifted
5. Move on

Steps can be reordered if real usage reveals a different priority. The
numbering is a suggested sequence, not a contract. The only rule is: each step
should leave the engine better than it found it.
