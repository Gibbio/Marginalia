"""Local search workflows."""

from __future__ import annotations

from marginalia_core.application.result import OperationResult
from marginalia_core.ports.storage import DocumentRepository, NoteRepository


class SearchService:
    """Search document and note repositories."""

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        note_repository: NoteRepository,
    ) -> None:
        self._document_repository = document_repository
        self._note_repository = note_repository

    def search_documents(self, query: str) -> OperationResult:
        if not query.strip():
            return OperationResult.error("A non-empty search query is required.")
        results = self._document_repository.search_documents(query)
        return OperationResult.ok(
            "Document search completed.",
            data={"query": query, "results": list(results)},
        )

    def search_notes(self, query: str) -> OperationResult:
        if not query.strip():
            return OperationResult.error("A non-empty search query is required.")
        results = self._note_repository.search_notes(query)
        return OperationResult.ok(
            "Note search completed.",
            data={"query": query, "results": list(results)},
        )
