# marginalia-import-epub

EPUB 2 / EPUB 3 importer. Pure Rust — no native dependencies.

## How it works

1. Open the `.epub` archive with the `epub` crate (ZIP + XHTML parser).
2. Walk the spine in reading order. Each spine item becomes one
   `ImportedSection`.
3. For each item, pull the XHTML and extract block-level text with
   `scraper` (CSS selector `p, h1..h6, li, blockquote`). Scripts, styles,
   and images are ignored.
4. Map section titles from the TOC by matching spine item paths against
   `NavPoint.content` (fragments stripped). Fallback is `Chapter N`.
5. The book's `dc:title` metadata becomes the `ImportedDocument.title`.

Paragraphs shorter than 15 characters are dropped (page numbers,
decorative dividers, stray markup).

Long paragraphs and paragraph-level sizing are handled by the core
chunker (`marginalia-core::domain::chunk_section_text`).

## Not handled

- Images (no OCR, no alt text extraction yet)
- MathML
- CSS-driven pagination / reflow hints
