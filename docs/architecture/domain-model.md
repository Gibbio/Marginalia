# Domain Model

## Document

An ingested local text artifact. A document contains ordered sections and each
section contains ordered chunks. This gives Marginalia stable anchors for
playback, notes, chapter navigation, and later rewrite or summary tasks.

## Section / Chapter / Chunk

- section: user-meaningful subdivision, usually a chapter
- chunk: the smallest addressable reading unit for playback and note anchoring

The current parser keeps this simple by deriving sections from markdown-style
headings and chunks from paragraph breaks. The persistence layer stores both the
full document outline and normalized section/chunk rows.

## Reading Session

Tracks:

- active document
- current reader state
- projected playback state
- current reading position
- last command executed
- active note capture id if one exists

The reading session is the backbone for pause/resume, note anchoring, rewrite
context, and coherent host behavior across restarts, reconnections, and future
platform shells.

## Voice Note

A voice note belongs to:

- a document
- a reading session
- a specific reading position

It also records:

- the final transcript text
- the transcription provider name
- the transcript language

This is a core distinction from generic note-taking. The note is useful because
it remembers where it came from and how it was captured.

## Rewrite Draft

A rewrite draft represents a derived text artifact for a specific section. It is
not the same as the original section and remains traceable to:

- the source section index
- the source anchor inside the document
- the note transcripts that informed it
- the provider that generated it

This is enough for a serious V0 rewrite flow without pretending that review or
acceptance workflow already exists.

## Summary Request / Result

Summaries are modeled explicitly so they can later support:

- per-document summaries
- corpus-level topic summaries
- cached or persisted summary outputs

The current result model already stores:

- summary text
- matched document ids
- generated highlights
- provider identity

## Search Query / Result

Search requests are modeled explicitly through `SearchQuery`, which currently
supports:

- free-text query text
- optional document scoping
- result limit

A search result is intentionally generic. It can represent a hit from:

- a document chunk
- a note
- later, a rewrite draft or summary cache

This keeps search UX separate from storage implementation details.
