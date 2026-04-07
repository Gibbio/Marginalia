"""Stdio transport for frontend/backend communication."""

from __future__ import annotations

import json
import logging
import sys
import traceback
from time import perf_counter
from dataclasses import asdict

from marginalia_backend.gateway import LocalFrontendGateway
from marginalia_backend.serialization import to_transport_value
from marginalia_core.application.frontend.envelopes import (
    FrontendRequest,
    FrontendResponse,
    FrontendResponseStatus,
)

logger = logging.getLogger(__name__)


class StdioFrontendServer:
    """Serve frontend requests over stdio using JSON Lines."""

    def __init__(self, gateway: LocalFrontendGateway) -> None:
        self._gateway = gateway

    def serve_forever(self) -> None:
        """Read requests from stdin and write responses to stdout."""

        for raw_line in sys.stdin:
            line = raw_line.strip()
            if not line:
                continue
            response = self._handle_line(line)
            self._write_response(response)

    def _handle_line(self, line: str) -> FrontendResponse:
        started_at = perf_counter()
        try:
            decoded = json.loads(line)
        except json.JSONDecodeError as exc:
            return FrontendResponse(
                status=FrontendResponseStatus.ERROR,
                name="invalid_json",
                message=f"Invalid JSON request: {exc.msg}",
            )

        try:
            request = FrontendRequest.from_dict(decoded)
        except ValueError as exc:
            raw_id = decoded.get("id") if isinstance(decoded, dict) else None
            return FrontendResponse(
                status=FrontendResponseStatus.ERROR,
                name="invalid_request",
                message=str(exc),
                request_id=str(raw_id) if raw_id is not None else None,
            )

        try:
            if request.request_type == "command":
                response = self._gateway.execute_command(request)
                self._log_request_timing(request.request_type, request.name, response, started_at)
                return response
            if request.request_type == "query":
                response = self._gateway.execute_query(request)
                self._log_request_timing(request.request_type, request.name, response, started_at)
                return response
        except Exception as exc:  # pragma: no cover - defensive transport guard
            traceback.print_exc(file=sys.stderr)
            return FrontendResponse(
                status=FrontendResponseStatus.ERROR,
                name=request.name,
                message=f"Backend request failed: {exc.__class__.__name__}: {exc}",
                request_id=request.request_id,
            )
        return FrontendResponse(
            status=FrontendResponseStatus.ERROR,
            name=request.name,
            message=f"Unknown request type: {request.request_type}",
            request_id=request.request_id,
        )

    @staticmethod
    def _log_request_timing(
        request_type: str,
        name: str,
        response: FrontendResponse,
        started_at: float,
    ) -> None:
        total_ms = (perf_counter() - started_at) * 1000
        if request_type != "command" and total_ms < 200:
            return
        logger.info(
            "timing frontend_request type=%s name=%s status=%s total_ms=%.2f",
            request_type,
            name,
            response.status.value,
            total_ms,
        )

    def _write_response(self, response: FrontendResponse) -> None:
        payload = to_transport_value(asdict(response))
        sys.stdout.write(json.dumps(payload))
        sys.stdout.write("\n")
        sys.stdout.flush()
