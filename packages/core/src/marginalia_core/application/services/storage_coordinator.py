"""Storage-focused workflows such as document ingestion."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.document import build_document_outline
from marginalia_core.ports.storage import DocumentRepository


class StorageCoordinationService:
    """Coordinate file ingestion into local storage."""

    def __init__(self, *, document_repository: DocumentRepository) -> None:
        self._document_repository = document_repository

    def ingest_text_file(self, source_path: Path) -> OperationResult:
        if source_path.suffix.lower() not in {".md", ".markdown", ".txt"}:
            return OperationResult.error(
                "Only plain text and markdown ingestion are supported in the bootstrap."
            )

        raw_text = source_path.read_text(encoding="utf-8")
        document = build_document_outline(source_path, raw_text)
        self._document_repository.save_document(document)
        return OperationResult.ok(
            "Document ingested into local SQLite storage.",
            data={"document": document},
        )
