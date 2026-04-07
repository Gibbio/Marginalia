"""Stable frontend query names."""

from __future__ import annotations

from enum import Enum


class FrontendQueryName(str, Enum):
    """Queries that read backend state."""

    GET_APP_SNAPSHOT = "get_app_snapshot"
    GET_BACKEND_CAPABILITIES = "get_backend_capabilities"
    GET_DOCTOR_REPORT = "get_doctor_report"
    GET_SESSION_SNAPSHOT = "get_session_snapshot"
    LIST_DOCUMENTS = "list_documents"
