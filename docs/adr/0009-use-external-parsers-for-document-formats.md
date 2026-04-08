# ADR 0009: Use External Parsers for Document Formats

- Status: Accepted

## Context

Marginalia ingests text documents and splits them into sections and chunks
for audio playback. The original implementation used hand-rolled regex and
string splitting to handle markdown specifics like headings, thematic breaks
(`---`, `***`) and paragraph boundaries.

This approach had growing blind spots:

- YAML frontmatter (`---\ntitle: ...\n---`) leaked into readable chunks.
- `#` characters inside fenced code blocks were misidentified as headings.
- Thematic break detection required a dedicated regex that only covered
  markdown files, not other formats.
- Each new format quirk demanded another special case, making the parser
  increasingly fragile and hard to reason about.

## Decision

Delegate format-aware parsing to established external libraries instead of
reimplementing format knowledge.

For markdown, use **markdown-it-py** (CommonMark-compliant parser) with the
**mdit-py-plugins** frontmatter extension. The token stream structurally
distinguishes headings, prose paragraphs, code fences, thematic breaks and
frontmatter — no regex heuristics needed.

Plain-text files keep a simple paragraph-based splitter since there is no
format structure to parse.

The chunking and merging pipeline downstream of parsing is unchanged.

## Consequences

- Frontmatter, code blocks, thematic breaks and other non-prose elements
  are excluded structurally rather than by pattern matching.
- Adding support for a new format (e.g. `.rst`, `.epub`) means wiring in
  another parser that feeds the same section/paragraph interface, not
  writing more regex.
- Two new runtime dependencies: `markdown-it-py` and `mdit-py-plugins`.
- Developers should prefer proven parsing libraries over custom string
  manipulation when dealing with document formats.

## Alternatives Considered

- Expanding the regex approach: rejected because every new edge case added
  fragility and the frontmatter/code-fence problems required structural
  awareness that regex cannot provide.
- `mistune`: viable, but `markdown-it-py` has a richer plugin ecosystem
  (frontmatter, footnotes, etc.) and closer CommonMark conformance.
