"""Document ingestion workflows."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.document import build_document_outline
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.storage import DocumentRepository


class DocumentIngestionService:
    """Coordinate local document ingestion and persistence."""

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        event_publisher: EventPublisher,
    ) -> None:
        self._document_repository = document_repository
        self._event_publisher = event_publisher

    def ingest_text_file(self, source_path: Path) -> OperationResult:
        if source_path.suffix.lower() not in {".md", ".markdown", ".txt"}:
            return OperationResult.error(
                "Only plain text and markdown ingestion are supported in V0."
            )

        raw_text = source_path.read_text(encoding="utf-8").strip()
        if not raw_text:
            return OperationResult.error("The source file is empty.")

        document = build_document_outline(source_path, raw_text)
        existing = self._document_repository.get_document(document.document_id)
        self._document_repository.save_document(document)
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.DOCUMENT_INGESTED,
                payload={
                    "document_id": document.document_id,
                    "title": document.title,
                    "chapter_count": document.chapter_count,
                    "chunk_count": document.total_chunk_count,
                    "already_present": existing is not None,
                },
            )
        )
        return OperationResult.ok(
            "Document ingested into local SQLite storage.",
            data={
                "document": document,
                "already_present": existing is not None,
                "stats": {
                    "chapter_count": document.chapter_count,
                    "chunk_count": document.total_chunk_count,
                },
            },
        )
