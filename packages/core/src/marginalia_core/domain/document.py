"""Document and chapter models."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import UTC, datetime
from hashlib import sha256
from pathlib import Path


def _utc_now() -> datetime:
    return datetime.now(UTC)


@dataclass(frozen=True, slots=True)
class DocumentChunk:
    """Smallest unit the reader may anchor playback or notes to."""

    index: int
    text: str
    char_start: int
    char_end: int


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


def build_document_outline(source_path: Path, raw_text: str) -> Document:
    """Parse a text document into sections and chunks.

    The parser is intentionally simple for bootstrap use:

    - markdown headings (`#`, `##`, `###`) define section boundaries
    - if no headings are present, a single section is created
    - blank lines split paragraph chunks inside a section
    """

    cleaned_text = raw_text.strip()
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
                chunks=_chunk_section_text(section_text),
                source_anchor=f"section:{len(sections)}",
            )
        )
        current_title = None
        current_lines = []

    for line in cleaned_text.splitlines():
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
                chunks=_chunk_section_text(cleaned_text),
                source_anchor="section:0",
            )
        ]

    document_id = sha256(f"{source_path.resolve()}::{cleaned_text}".encode("utf-8")).hexdigest()[:12]
    return Document(
        document_id=document_id,
        title=title,
        source_path=source_path.resolve(),
        sections=tuple(sections),
    )


def _chunk_section_text(section_text: str) -> tuple[DocumentChunk, ...]:
    paragraphs = [paragraph.strip() for paragraph in section_text.split("\n\n") if paragraph.strip()]
    if not paragraphs:
        return (
            DocumentChunk(
                index=0,
                text="",
                char_start=0,
                char_end=0,
            ),
        )

    chunks: list[DocumentChunk] = []
    search_start = 0
    for index, paragraph in enumerate(paragraphs):
        char_start = section_text.find(paragraph, search_start)
        if char_start < 0:
            char_start = search_start
        char_end = char_start + len(paragraph)
        chunks.append(
            DocumentChunk(
                index=index,
                text=paragraph,
                char_start=char_start,
                char_end=char_end,
            )
        )
        search_start = char_end
    return tuple(chunks)
