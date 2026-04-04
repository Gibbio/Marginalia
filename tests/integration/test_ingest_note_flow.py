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

    note_start_result = runner.invoke(app, ["note-start", "--json"], env=env)
    assert note_start_result.exit_code == 0

    note_stop_result = runner.invoke(
        app,
        ["note-stop", "--text", "Add a sharper explanation here.", "--json"],
        env=env,
    )
    assert note_stop_result.exit_code == 0

    search_result = runner.invoke(app, ["search-notes", "sharper", "--json"], env=env)
    assert search_result.exit_code == 0
    payload = json.loads(search_result.stdout)
    assert len(payload["data"]["results"]) == 1
