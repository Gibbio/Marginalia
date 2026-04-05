# Domain Model

## Document

An ingested local text artifact. A document contains ordered sections and each
section contains ordered chunks. This gives Marginalia stable anchors for
playback, notes, and later rewrite or summary tasks.

## Section / Chapter / Chunk

- section: user-meaningful subdivision, often a chapter
- chunk: the smallest addressable playback or note anchor

The current parser keeps this simple by deriving sections from markdown-style
headings and chunks from paragraph breaks.

## Reading Session

Tracks:

- active document
- current state
- playback state
- current position
- last command
- active note capture id if one exists

The current implementation persists:

- reader state
- playback state
- current section and chunk
- last command
- active note capture id

The reading session is the backbone for pause/resume, note anchoring, and later
rewrite context.

## Voice Note

A voice note belongs to:

- a document
- a reading session
- a specific reading position

This is a core distinction from generic note-taking. The note is useful because
it remembers where it came from.

## Rewrite Draft

A rewrite draft represents a derived text artifact for a specific section. It is
not the same as the original section and should remain traceable to:

- the source section
- the note transcripts that informed it
- the provider used to generate it later

## Summary Request / Result

Summaries are modeled explicitly so they can later support:

- per-document summaries
- corpus-level topic summaries
- cached or persisted summary outputs

The V0 implementation already models summary requests explicitly even though the
provider remains fake.

## Search Query / Result

Search requests are modeled explicitly through `SearchQuery`, which currently
supports:

- free-text query text
- optional document scoping
- result limit

A search result is intentionally generic. It can represent a hit from:

- a document
- a note
- later, a rewrite draft or summary cache

This keeps search UX separate from storage implementation details.
