"""Document and chapter models."""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path

_DEFAULT_CHUNK_TARGET_CHARS = 300
_SENTENCE_BOUNDARY = re.compile(r"(?<=[.!?…])\s+")
_MARKDOWN_THEMATIC_BREAK = re.compile(r"^\s{0,3}([-*_])(?:\s*\1){2,}\s*$")


def _utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class DocumentChunk:
    """Smallest unit the reader may anchor playback or notes to."""

    index: int
    text: str
    char_start: int
    char_end: int

    @property
    def anchor(self) -> str:
        return f"chunk:{self.index}"


@dataclass(frozen=True, slots=True)
class DocumentSection:
    """Logical section or chapter of a document."""

    index: int
    title: str
    chunks: tuple[DocumentChunk, ...]
    source_anchor: str | None = None

    @property
    def text(self) -> str:
        return "\n\n".join(chunk.text for chunk in self.chunks)

    @property
    def chunk_count(self) -> int:
        return len(self.chunks)

    def get_chunk(self, chunk_index: int) -> DocumentChunk:
        return self.chunks[chunk_index]


@dataclass(frozen=True, slots=True)
class Document:
    """Persistent representation of an ingested document."""

    document_id: str
    title: str
    source_path: Path
    sections: tuple[DocumentSection, ...]
    imported_at: datetime = field(default_factory=_utc_now)

    @property
    def chapter_count(self) -> int:
        return len(self.sections)

    @property
    def total_chunk_count(self) -> int:
        return sum(section.chunk_count for section in self.sections)

    def get_section(self, section_index: int) -> DocumentSection:
        return self.sections[section_index]

    def get_chunk(self, section_index: int, chunk_index: int) -> DocumentChunk:
        return self.get_section(section_index).get_chunk(chunk_index)


def build_document_outline(
    source_path: Path,
    raw_text: str,
    *,
    chunk_target_chars: int = _DEFAULT_CHUNK_TARGET_CHARS,
) -> Document:
    """Parse a text document into sections and chunks.

    - markdown headings (``#``, ``##``, ``###``) define section boundaries
    - if no headings are present, a single section is created
    - paragraphs longer than 1.5x *chunk_target_chars* are split at sentence
      boundaries
    - short consecutive fragments are merged until they approach the target
    """

    cleaned_text = raw_text.strip()
    markdown_source = source_path.suffix.lower() in {".md", ".markdown"}
    normalized_text = (
        _strip_markdown_thematic_breaks(cleaned_text) if markdown_source else cleaned_text
    )
    title = source_path.stem.replace("-", " ").replace("_", " ").title() or "Untitled Document"
    sections: list[DocumentSection] = []
    current_title: str | None = None
    current_lines: list[str] = []

    def flush_section() -> None:
        nonlocal current_title, current_lines
        if current_title is None and not current_lines:
            return
        section_title = current_title or f"Section {len(sections) + 1}"
        section_text = "\n".join(current_lines).strip()
        sections.append(
            DocumentSection(
                index=len(sections),
                title=section_title,
                chunks=_chunk_section_text(
                    section_text, chunk_target_chars=chunk_target_chars
                ),
                source_anchor=f"section:{len(sections)}",
            )
        )
        current_title = None
        current_lines = []

    for line in normalized_text.splitlines():
        if line.lstrip().startswith("#"):
            flush_section()
            current_title = line.lstrip("#").strip() or f"Section {len(sections) + 1}"
            continue
        current_lines.append(line)

    flush_section()

    if not sections:
        sections = [
            DocumentSection(
                index=0,
                title=title,
                chunks=_chunk_section_text(
                    normalized_text, chunk_target_chars=chunk_target_chars
                ),
                source_anchor="section:0",
            )
        ]

    document_hash_input = f"{source_path.resolve()}::{normalized_text}".encode()
    document_id = sha256(document_hash_input).hexdigest()[:12]
    return Document(
        document_id=document_id,
        title=title,
        source_path=source_path.resolve(),
        sections=tuple(sections),
    )


# ---------------------------------------------------------------------------
# Chunking
# ---------------------------------------------------------------------------


def _chunk_section_text(
    section_text: str,
    *,
    chunk_target_chars: int = _DEFAULT_CHUNK_TARGET_CHARS,
) -> tuple[DocumentChunk, ...]:
    """Split section text into reading-sized chunks.

    1. Locate paragraphs with their character offsets.
    2. Split long paragraphs at sentence boundaries.
    3. Greedily merge small consecutive fragments until the target is reached.
    """

    fragments = _locate_paragraphs(section_text)
    if not fragments:
        return (DocumentChunk(index=0, text="", char_start=0, char_end=0),)

    split_threshold = int(chunk_target_chars * 1.5)
    expanded: list[tuple[str, int, int]] = []
    for text, start, end in fragments:
        if len(text) > split_threshold:
            expanded.extend(_split_at_sentences(text, start))
        else:
            expanded.append((text, start, end))

    merged = _merge_fragments(expanded, chunk_target_chars)
    return tuple(
        DocumentChunk(index=i, text=text, char_start=start, char_end=end)
        for i, (text, start, end) in enumerate(merged)
    )


def _strip_markdown_thematic_breaks(text: str) -> str:
    """Remove standalone markdown thematic breaks from ingested content."""

    return "\n".join(
        line for line in text.splitlines() if not _MARKDOWN_THEMATIC_BREAK.match(line)
    ).strip()


def _locate_paragraphs(section_text: str) -> list[tuple[str, int, int]]:
    """Return (text, char_start, char_end) for each non-empty paragraph."""

    result: list[tuple[str, int, int]] = []
    search_start = 0
    for raw in section_text.split("\n\n"):
        stripped = raw.strip()
        if not stripped:
            search_start += len(raw) + 2
            continue
        char_start = section_text.find(stripped, search_start)
        if char_start < 0:
            char_start = search_start
        char_end = char_start + len(stripped)
        result.append((stripped, char_start, char_end))
        search_start = char_end
    return result


def _split_at_sentences(
    text: str, base_offset: int
) -> list[tuple[str, int, int]]:
    """Split a long paragraph at sentence-ending punctuation."""

    parts = _SENTENCE_BOUNDARY.split(text)
    result: list[tuple[str, int, int]] = []
    search_start = 0
    for part in parts:
        stripped = part.strip()
        if not stripped:
            continue
        pos = text.find(stripped, search_start)
        if pos < 0:
            pos = search_start
        result.append((stripped, base_offset + pos, base_offset + pos + len(stripped)))
        search_start = pos + len(stripped)
    return result or [(text, base_offset, base_offset + len(text))]


def _merge_fragments(
    fragments: list[tuple[str, int, int]],
    target: int,
) -> list[tuple[str, int, int]]:
    """Greedily merge consecutive fragments that fit within *target* chars."""

    merged: list[tuple[str, int, int]] = []
    buf_texts: list[str] = []
    buf_start = 0
    buf_end = 0
    buf_len = 0

    for text, start, end in fragments:
        addition = len(text) + (1 if buf_texts else 0)
        if buf_texts and buf_len + addition > target:
            merged.append((" ".join(buf_texts), buf_start, buf_end))
            buf_texts = [text]
            buf_start = start
            buf_end = end
            buf_len = len(text)
        else:
            if not buf_texts:
                buf_start = start
            buf_texts.append(text)
            buf_end = end
            buf_len += addition

    if buf_texts:
        merged.append((" ".join(buf_texts), buf_start, buf_end))

    return merged
