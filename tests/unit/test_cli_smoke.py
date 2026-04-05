"""CLI smoke tests for the single supported runtime mode."""

from __future__ import annotations

import json
from pathlib import Path

from typer.testing import CliRunner

from marginalia_cli.main import app


def test_doctor_command_reports_command_lexicon_and_schema(tmp_path: Path) -> None:
    runner = CliRunner()
    result = runner.invoke(
        app,
        ["doctor", "--json"],
        env={
            "MARGINALIA_DB_PATH": str(tmp_path / "doctor.sqlite3"),
            "MARGINALIA_TTS_PROVIDER": "fake",
            "MARGINALIA_PLAYBACK_PROVIDER": "fake",
        },
    )

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["status"] == "ok"
    assert payload["data"]["database"]["schema_version"] == "4"
    assert payload["data"]["database"]["schema_profile"] == "sqlite-v4-migrated"
    assert payload["data"]["command_lexicon"]["language"] == "it"


def test_play_command_auto_ingests_file_and_completes_runtime(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "play.sqlite3"),
        "MARGINALIA_TTS_PROVIDER": "fake",
        "MARGINALIA_PLAYBACK_PROVIDER": "fake",
        "MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS": "0",
    }

    result = runner.invoke(app, ["play", str(source_path), "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["runtime"]["outcome"] == "completed"
    assert payload["data"]["target"]["ingested_now"] is True
    assert payload["data"]["session"]["runtime_status"] == "completed"
    assert payload["data"]["runtime_details"]["command_language"] == "it"


def test_play_command_dispatches_fake_voice_commands_during_runtime(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "runtime.sqlite3"),
        "MARGINALIA_FAKE_COMMANDS": "pausa,continua,stop",
        "MARGINALIA_TTS_PROVIDER": "fake",
        "MARGINALIA_PLAYBACK_PROVIDER": "fake",
        "MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS": "2",
    }

    result = runner.invoke(app, ["play", str(source_path), "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["runtime"]["outcome"] == "stopped"
    assert payload["data"]["runtime"]["handled_command_count"] == 3
    assert payload["data"]["runtime"]["handled_commands"][0]["handled_command"] == "pause"
    assert payload["data"]["runtime"]["handled_commands"][1]["handled_command"] == "resume"
    assert payload["data"]["runtime"]["handled_commands"][2]["handled_command"] == "stop"

    status_result = runner.invoke(app, ["status", "--json"], env=env)
    assert status_result.exit_code == 0
    status_payload = json.loads(status_result.stdout)
    assert status_payload["data"]["runtime"]["command_listening_active"] is False
    assert status_payload["data"]["runtime"]["runtime_status"] == "stopped"
    assert status_payload["data"]["runtime_details"]["command_language"] == "it"


def test_stop_command_reports_cleanup_even_without_active_runtime(tmp_path: Path) -> None:
    runner = CliRunner()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "stop.sqlite3"),
        "MARGINALIA_TTS_PROVIDER": "fake",
        "MARGINALIA_PLAYBACK_PROVIDER": "fake",
    }

    result = runner.invoke(app, ["stop", "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["status"] == "ok"
    assert payload["data"]["cleanup"]["cleaned_up"] is False
