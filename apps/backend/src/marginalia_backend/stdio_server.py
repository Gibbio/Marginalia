"""Stdio transport for frontend/backend communication."""

from __future__ import annotations

import json
import sys
from dataclasses import asdict

from marginalia_backend.gateway import LocalFrontendGateway
from marginalia_backend.serialization import to_transport_value
from marginalia_core.application.frontend.envelopes import (
    FrontendRequest,
    FrontendResponse,
    FrontendResponseStatus,
)


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
            return FrontendResponse(
                status=FrontendResponseStatus.ERROR,
                name="invalid_request",
                message=str(exc),
            )

        if request.request_type == "command":
            return self._gateway.execute_command(request)
        if request.request_type == "query":
            return self._gateway.execute_query(request)
        return FrontendResponse(
            status=FrontendResponseStatus.ERROR,
            name=request.name,
            message=f"Unknown request type: {request.request_type}",
            request_id=request.request_id,
        )

    def _write_response(self, response: FrontendResponse) -> None:
        payload = to_transport_value(asdict(response))
        sys.stdout.write(json.dumps(payload))
        sys.stdout.write("\n")
        sys.stdout.flush()
