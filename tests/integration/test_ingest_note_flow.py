"""End-to-end local workflow tests."""

from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from marginalia_cli.main import app


def test_ingest_play_note_and_search_flow(tmp_path: Path) -> None:
    runner = CliRunner()
    database_path = tmp_path / "workflow.sqlite3"
    env = {"MARGINALIA_DB_PATH": str(database_path)}
    source_path = Path("tests/fixtures/sample_document.txt").resolve()

    ingest_result = runner.invoke(app, ["ingest", str(source_path), "--json"], env=env)
    assert ingest_result.exit_code == 0
    document_id = json.loads(ingest_result.stdout)["data"]["document"]["document_id"]

    play_result = runner.invoke(app, ["play", document_id, "--json"], env=env)
    assert play_result.exit_code == 0
    play_payload = json.loads(play_result.stdout)
    assert play_payload["data"]["playback"]["state"] == "playing"

    pause_result = runner.invoke(app, ["pause", "--json"], env=env)
    assert pause_result.exit_code == 0
    pause_payload = json.loads(pause_result.stdout)
    assert pause_payload["data"]["playback"]["state"] == "paused"

    note_start_result = runner.invoke(app, ["note-start", "--json"], env=env)
    assert note_start_result.exit_code == 0

    note_stop_result = runner.invoke(
        app,
        ["note-stop", "--text", "Add a sharper explanation here.", "--json"],
        env=env,
    )
    assert note_stop_result.exit_code == 0
    note_payload = json.loads(note_stop_result.stdout)
    assert note_payload["data"]["note"]["transcription_provider"] == "cli-manual"

    search_result = runner.invoke(app, ["search-notes", "sharper", "--json"], env=env)
    assert search_result.exit_code == 0
    payload = json.loads(search_result.stdout)
    assert len(payload["data"]["results"]) == 1

    rewrite_result = runner.invoke(app, ["rewrite-current", "--json"], env=env)
    assert rewrite_result.exit_code == 0
    rewrite_payload = json.loads(rewrite_result.stdout)
    assert rewrite_payload["data"]["draft"]["draft_id"]
    assert rewrite_payload["data"]["rewrite_output"]["provider_name"] == "fake-rewrite-llm"

    summary_result = runner.invoke(app, ["summarize-topic", "local", "--json"], env=env)
    assert summary_result.exit_code == 0
    summary_payload = json.loads(summary_result.stdout)
    assert summary_payload["data"]["summary"]["topic"] == "local"
    assert summary_payload["data"]["summary"]["provider_name"] == "fake-summary-llm"

    document_search_result = runner.invoke(app, ["search-document", "local", "--json"], env=env)
    assert document_search_result.exit_code == 0
    document_search_payload = json.loads(document_search_result.stdout)
    assert len(document_search_payload["data"]["results"]) == 1
    assert document_search_payload["data"]["results"][0]["anchor"].startswith("section:")

    status_result = runner.invoke(app, ["status", "--json"], env=env)
    assert status_result.exit_code == 0
    status_payload = json.loads(status_result.stdout)
    assert status_payload["data"]["counts"]["notes"] == 1
    assert status_payload["data"]["counts"]["drafts"] == 1
    assert status_payload["data"]["counts"]["notes_in_current_section"] == 1
    assert status_payload["data"]["session"]["state"] == "PAUSED"
    assert status_payload["data"]["latest_draft"]["provider_name"] == "fake-rewrite-llm"
