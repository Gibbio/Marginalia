"""Local search workflows."""

from __future__ import annotations

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.search import SearchQuery
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
        search_query = SearchQuery(text=query)
        if not search_query.normalized_text:
            return OperationResult.error("A non-empty search query is required.")
        results = self._document_repository.search_documents(search_query)
        return OperationResult.ok(
            "Document search completed.",
            data={"query": search_query, "results": list(results)},
        )

    def search_notes(self, query: str) -> OperationResult:
        search_query = SearchQuery(text=query)
        if not search_query.normalized_text:
            return OperationResult.error("A non-empty search query is required.")
        results = self._note_repository.search_notes(search_query)
        return OperationResult.ok(
            "Note search completed.",
            data={"query": search_query, "results": list(results)},
        )
