"""Tests for the headless backend frontend-gateway surface."""

from __future__ import annotations

import json
from pathlib import Path

from pytest import MonkeyPatch
from typer.testing import CliRunner

from marginalia_backend.bootstrap import build_backend_container
from marginalia_backend.gateway import LocalFrontendGateway
from marginalia_backend.main import app
from marginalia_core.application.frontend.envelopes import FrontendRequest


def _build_gateway(tmp_path: Path, monkeypatch: MonkeyPatch) -> LocalFrontendGateway:
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "backend.sqlite3"))
    monkeypatch.setenv("MARGINALIA_TTS_PROVIDER", "fake")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_PROVIDER", "fake")
    container = build_backend_container()
    return LocalFrontendGateway(container)


def test_get_backend_capabilities_query_reports_supported_contract(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)

    response = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_backend_capabilities")
    )

    assert response.status.value == "ok"
    assert "start_session" in response.payload["commands"]
    assert "get_app_snapshot" in response.payload["queries"]
    assert response.payload["protocol_version"] == 1


def test_ingest_document_command_then_list_documents_query(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    ingest_response = gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )
    list_response = gateway.execute_query(
        FrontendRequest(request_type="query", name="list_documents")
    )

    assert ingest_response.status.value == "ok"
    assert list_response.status.value == "ok"
    assert len(list_response.payload["documents"]) == 1
    assert list_response.payload["documents"][0]["title"] == "Sample Document"


def test_backend_stdio_server_handles_capabilities_query(tmp_path: Path) -> None:
    runner = CliRunner()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "stdio.sqlite3"),
        "MARGINALIA_TTS_PROVIDER": "fake",
        "MARGINALIA_PLAYBACK_PROVIDER": "fake",
    }
    payload = {
        "type": "query",
        "name": "get_backend_capabilities",
        "payload": {},
        "id": "req-1",
    }

    result = runner.invoke(app, ["serve-stdio"], env=env, input=json.dumps(payload))

    assert result.exit_code == 0
    response = json.loads(result.stdout.strip())
    assert response["request_id"] == "req-1"
    assert response["status"] == "ok"
    assert "stdio-jsonl" in response["payload"]["transports"]
