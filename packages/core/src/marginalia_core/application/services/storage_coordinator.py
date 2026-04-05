"""Backward-compatible storage coordination naming."""

from __future__ import annotations

from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)


class StorageCoordinationService(DocumentIngestionService):
    """Compatibility alias for the earlier bootstrap name."""
