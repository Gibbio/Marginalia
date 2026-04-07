"""Stable frontend query names."""

from __future__ import annotations

from enum import Enum


class FrontendQueryName(str, Enum):
    """Queries that read backend state."""

    GET_APP_SNAPSHOT = "get_app_snapshot"
    GET_BACKEND_CAPABILITIES = "get_backend_capabilities"
    GET_DOCUMENT_VIEW = "get_document_view"
    GET_DOCTOR_REPORT = "get_doctor_report"
    GET_SESSION_SNAPSHOT = "get_session_snapshot"
    LIST_NOTES = "list_notes"
    LIST_DOCUMENTS = "list_documents"
    SEARCH_DOCUMENTS = "search_documents"
    SEARCH_NOTES = "search_notes"
