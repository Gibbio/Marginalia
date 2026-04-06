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
- Voice commands: pause, resume, repeat, chapter nav, stop, help
- Document ingestion, session persistence, note anchoring
- Runtime supervision with PID reuse protection and file locking
- 65 tests, clean lint and types

Stubbed:
- Note dictation (returns fixed text)
- Rewrite and summary generation (fake providers)
- Desktop UI, editor adapters

Main rough edges:
- Chunking is paragraph-only — long paragraphs produce awkward playback units
- No way to inspect documents, notes, or drafts from the CLI
- Real-provider setup requires too many manual steps
- No progress indication during reading
- No way to list or manage sessions

---

## Step 1 — CLI Inspection Commands

Make stored data accessible without opening SQLite.

- `list-documents` — show ingested documents with title, section count, chunk
  count, import date
- `show-document <id>` — show document outline (sections and chunk counts)
- `list-notes <document-id>` — show anchored notes for a document
- `list-drafts <document-id>` — show rewrite drafts for a document
- `list-sessions` — show recent sessions with state, document, last command
- output stays JSON-structured and script-friendly
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

## Step 3 — Provider Setup Ergonomics

Reduce friction for first-time real-provider setup on macOS.

- `doctor` should print actionable remediation hints, not just readiness flags
  (e.g. "run `make bootstrap-kokoro` to set up Kokoro TTS")
- `doctor` should report the detected default audio input/output device names
  clearly, not just indices
- `doctor` should warn when `allow_fallback = true` and real providers are not
  ready (currently silent)
- Add a `setup` section to `doctor` output: a checklist of what is ready and
  what is missing, ordered by setup priority
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

- `list-sessions` already added in Step 1 — extend it here
- `delete-session <id>` — deactivate and remove a session
- `resume-session <id>` — switch the active session to a previous one
- When starting `play` with no target and no active session, show a list of
  recent sessions the user can resume instead of just picking the latest
  document
- Session history is useful even with a single-user CLI

Size: small. Query and lifecycle commands, no new domain concepts.

## Step 7 — Real Note Dictation [partial]

Completed April 2026 (whisper.cpp adapter).

- `WhisperCppDictationTranscriber` adapter: records from mic via
  `sounddevice`, invokes whisper.cpp `main` binary, parses output
- `[whisper_cpp]` config section with `executable`, `model_path`,
  `language`, `max_record_seconds`
- `doctor` reports whisper-cpp readiness
- bootstrap wires `dictation_stt = "whisper-cpp"` with fallback
- `make bootstrap-whisper` builds whisper.cpp and downloads the base model
- 9 new tests

Remaining:
- Raw audio path storage for later review
- Smoke-test with real hardware
- README installation section update for whisper setup

## Step 8 — Note Review and Editing

Make captured notes actionable.

- `show-note <id>` — display full note transcript with its anchor position
- `edit-note <id> --transcript "..."` — correct a dictation mistake
- `delete-note <id>` — remove a note
- `play-note <id>` — play back the raw audio if it was saved
- Notes should display their position context: which section title and chunk
  excerpt they are anchored to

Size: small. CLI surface and repository queries.

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

## Step 12 — Real Summarization

Replace the fake summarization provider with a real one.

- Reuse the same LLM backend from Step 10
- The summary port (`TopicSummarizer`) already exists
- Summary results should be persisted (currently transient) — add a `summaries`
  table via migration
- `list-summaries <document-id>` and `show-summary <id>`

Size: small-medium. New migration, adapter reuse, CLI surface.

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

## Step 15 — Desktop Shell Spike

Add a thin desktop interface without changing core assumptions.

- Evaluate Textual (terminal UI) vs a lightweight native wrapper (e.g.
  Tauri with a Python backend, or a simple Tkinter panel)
- The `RuntimeLoop.step()` model already supports timer-driven callers
- The shell should show: current document/section/chunk, playback state,
  recent voice commands, reading progress
- Voice commands continue through the microphone — the shell adds visual
  feedback, not new input methods
- Keep it as a separate app in `apps/desktop/`

Size: medium-large. New app surface, but core and services are reused as-is.

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
