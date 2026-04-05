"""End-to-end runtime loop tests."""

from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from marginalia_cli.main import app


def test_play_runtime_ingests_handles_commands_and_updates_status(tmp_path: Path) -> None:
    runner = CliRunner()
    database_path = tmp_path / "workflow.sqlite3"
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {
        "MARGINALIA_DB_PATH": str(database_path),
        "MARGINALIA_FAKE_COMMANDS": "pausa,continua,stop",
        "MARGINALIA_TTS_PROVIDER": "fake",
        "MARGINALIA_PLAYBACK_PROVIDER": "fake",
        "MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS": "2",
    }

    play_result = runner.invoke(app, ["play", str(source_path), "--json"], env=env)
    assert play_result.exit_code == 0
    play_payload = json.loads(play_result.stdout)
    assert play_payload["data"]["runtime"]["outcome"] == "stopped"
    assert play_payload["data"]["runtime"]["handled_command_count"] == 3
    assert play_payload["data"]["target"]["ingested_now"] is True

    status_result = runner.invoke(app, ["status", "--json"], env=env)
    assert status_result.exit_code == 0
    status_payload = json.loads(status_result.stdout)
    assert status_payload["data"]["session"]["runtime_status"] == "stopped"
    assert status_payload["data"]["runtime"]["command_listening_active"] is False
    assert status_payload["data"]["runtime_details"]["command_language"] == "it"

    document_search_result = runner.invoke(app, ["search-document", "local", "--json"], env=env)
    assert document_search_result.exit_code == 0
    document_search_payload = json.loads(document_search_result.stdout)
    assert len(document_search_payload["data"]["results"]) == 1
