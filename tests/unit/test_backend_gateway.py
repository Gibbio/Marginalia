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


def test_document_view_query_reports_sections_and_active_chunk(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="start_session",
            payload={"target": str(source_path)},
        )
    )

    response = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_document_view")
    )

    assert response.status.value == "ok"
    document = response.payload["document"]
    assert document["chapter_count"] == 2
    assert document["active_section_index"] == 0
    assert document["sections"][0]["chunks"][0]["is_active"] is True


def test_list_notes_query_reports_created_note_for_active_document(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="start_session",
            payload={"target": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="create_note",
            payload={"text": "Remember this chapter."},
        )
    )

    response = gateway.execute_query(
        FrontendRequest(request_type="query", name="list_notes")
    )

    assert response.status.value == "ok"
    notes_snapshot = response.payload["notes"]
    assert len(notes_snapshot["notes"]) == 1
    assert notes_snapshot["notes"][0]["transcript"] == "Remember this chapter."


def test_restart_chapter_command_is_available_in_backend_contract(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)

    response = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_backend_capabilities")
    )

    assert response.status.value == "ok"
    assert "restart_chapter" in response.payload["commands"]


def test_navigation_commands_are_available_in_backend_contract(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)

    response = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_backend_capabilities")
    )

    assert response.status.value == "ok"
    assert "next_chunk" in response.payload["commands"]
    assert "previous_chapter" in response.payload["commands"]


def test_next_chunk_and_previous_chapter_commands_navigate_session(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="start_session",
            payload={"target": str(source_path)},
        )
    )
    next_chunk_response = gateway.execute_command(
        FrontendRequest(request_type="command", name="next_chunk")
    )
    session_after_next = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_session_snapshot")
    )
    previous_chapter_response = gateway.execute_command(
        FrontendRequest(request_type="command", name="previous_chapter")
    )
    session_after_previous = gateway.execute_query(
        FrontendRequest(request_type="query", name="get_session_snapshot")
    )

    assert next_chunk_response.status.value == "ok"
    assert session_after_next.payload["session"]["chunk_index"] == 1
    assert previous_chapter_response.status.value == "error"
    assert session_after_previous.payload["session"]["section_index"] == 0


def test_search_documents_query_returns_document_hits(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )

    response = gateway.execute_query(
        FrontendRequest(
            request_type="query",
            name="search_documents",
            payload={"query": "attentive"},
        )
    )

    assert response.status.value == "ok"
    search = response.payload["search"]
    assert search["query"] == "attentive"
    assert len(search["results"]) == 1
    assert search["results"][0]["entity_kind"] == "document"


def test_search_notes_query_returns_note_hits(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="ingest_document",
            payload={"path": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="start_session",
            payload={"target": str(source_path)},
        )
    )
    gateway.execute_command(
        FrontendRequest(
            request_type="command",
            name="create_note",
            payload={"text": "Attentive reminder for later."},
        )
    )

    response = gateway.execute_query(
        FrontendRequest(
            request_type="query",
            name="search_notes",
            payload={"query": "reminder"},
        )
    )

    assert response.status.value == "ok"
    search = response.payload["search"]
    assert search["query"] == "reminder"
    assert len(search["results"]) == 1
    assert search["results"][0]["entity_kind"] == "note"


def test_search_documents_query_rejects_empty_query(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    gateway = _build_gateway(tmp_path, monkeypatch)

    response = gateway.execute_query(
        FrontendRequest(
            request_type="query",
            name="search_documents",
            payload={"query": "   "},
        )
    )

    assert response.status.value == "error"
    assert "non-empty search query" in response.message.lower()


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
