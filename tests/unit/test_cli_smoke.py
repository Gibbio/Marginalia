"""CLI smoke tests."""

from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from marginalia_cli.main import app


def test_doctor_command_returns_json(tmp_path: Path) -> None:
    runner = CliRunner()
    result = runner.invoke(
        app,
        ["doctor", "--json"],
        env={"MARGINALIA_DB_PATH": str(tmp_path / "doctor.sqlite3")},
    )

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["status"] == "ok"
    assert payload["data"]["database"]["schema_version"] == "1"


def test_ingest_command_returns_document_id(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    result = runner.invoke(
        app,
        ["ingest", str(source_path), "--json"],
        env={"MARGINALIA_DB_PATH": str(tmp_path / "ingest.sqlite3")},
    )

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["document"]["document_id"]
    assert payload["data"]["stats"]["chapter_count"] == 2
