"""Document ingestion workflows."""

from __future__ import annotations

import logging
from time import perf_counter
from pathlib import Path

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.document import build_document_outline
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.storage import DocumentRepository

logger = logging.getLogger(__name__)


class DocumentIngestionService:
    """Coordinate local document ingestion and persistence."""

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        event_publisher: EventPublisher,
        chunk_target_chars: int = 300,
    ) -> None:
        self._document_repository = document_repository
        self._event_publisher = event_publisher
        self._chunk_target_chars = chunk_target_chars

    def ingest_text_file(self, source_path: Path) -> OperationResult:
        started_at = perf_counter()
        if source_path.suffix.lower() not in {".md", ".markdown", ".txt"}:
            return OperationResult.error(
                "Only plain text and markdown ingestion are supported in V0."
            )

        read_started_at = perf_counter()
        raw_text = source_path.read_text(encoding="utf-8").strip()
        read_ms = (perf_counter() - read_started_at) * 1000
        if not raw_text:
            return OperationResult.error("The source file is empty.")

        outline_started_at = perf_counter()
        document = build_document_outline(
            source_path, raw_text, chunk_target_chars=self._chunk_target_chars
        )
        outline_ms = (perf_counter() - outline_started_at) * 1000
        existing = self._document_repository.get_document(document.document_id)

        save_started_at = perf_counter()
        self._document_repository.save_document(document)
        save_ms = (perf_counter() - save_started_at) * 1000

        chunk_lengths = [
            len(chunk.text) for section in document.sections for chunk in section.chunks
        ]
        timings = {
            "read_ms": round(read_ms, 2),
            "outline_ms": round(outline_ms, 2),
            "save_ms": round(save_ms, 2),
            "total_ms": round((perf_counter() - started_at) * 1000, 2),
        }
        chunk_stats = {
            "raw_char_count": len(raw_text),
            "chapter_count": document.chapter_count,
            "chunk_count": document.total_chunk_count,
            "avg_chunk_chars": round(sum(chunk_lengths) / len(chunk_lengths), 2),
            "max_chunk_chars": max(chunk_lengths),
        }

        logger.info(
            "timing ingestion document=%s chars=%d chapters=%d chunks=%d avg_chunk_chars=%.2f "
            "max_chunk_chars=%d read_ms=%.2f outline_ms=%.2f save_ms=%.2f total_ms=%.2f "
            "already_present=%s",
            document.document_id,
            chunk_stats["raw_char_count"],
            chunk_stats["chapter_count"],
            chunk_stats["chunk_count"],
            chunk_stats["avg_chunk_chars"],
            chunk_stats["max_chunk_chars"],
            timings["read_ms"],
            timings["outline_ms"],
            timings["save_ms"],
            timings["total_ms"],
            existing is not None,
        )
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.DOCUMENT_INGESTED,
                payload={
                    "document_id": document.document_id,
                    "title": document.title,
                    "chapter_count": document.chapter_count,
                    "chunk_count": document.total_chunk_count,
                    "already_present": existing is not None,
                    "timings": timings,
                    "chunk_stats": chunk_stats,
                },
            )
        )
        return OperationResult.ok(
            "Document ingested into local SQLite storage.",
            data={
                "document": document,
                "already_present": existing is not None,
                "stats": chunk_stats,
                "timings": timings,
            },
        )
