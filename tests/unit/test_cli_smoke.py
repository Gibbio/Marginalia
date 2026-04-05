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
    assert payload["data"]["database"]["schema_version"] == "2"
    assert payload["data"]["database"]["schema_profile"] == "sqlite-v2"
    assert (
        payload["data"]["provider_capabilities"]["command_stt"]["provider_name"]
        == "fake-command-stt"
    )
    assert payload["data"]["provider_capabilities"]["playback"]["provider_name"] == "fake-playback"


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


def test_play_command_returns_synthesis_and_playback(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {"MARGINALIA_DB_PATH": str(tmp_path / "play.sqlite3")}

    ingest_result = runner.invoke(app, ["ingest", str(source_path), "--json"], env=env)
    document_id = json.loads(ingest_result.stdout)["data"]["document"]["document_id"]

    result = runner.invoke(app, ["play", document_id, "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["synthesis"]["provider_name"] == "fake-tts"
    assert payload["data"]["playback"]["state"] == "playing"


def test_repeat_resume_and_navigation_commands(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {"MARGINALIA_DB_PATH": str(tmp_path / "navigation.sqlite3")}

    ingest_result = runner.invoke(app, ["ingest", str(source_path), "--json"], env=env)
    document_id = json.loads(ingest_result.stdout)["data"]["document"]["document_id"]

    assert runner.invoke(app, ["play", document_id, "--json"], env=env).exit_code == 0

    repeat_result = runner.invoke(app, ["repeat", "--json"], env=env)
    assert repeat_result.exit_code == 0
    repeat_payload = json.loads(repeat_result.stdout)
    assert repeat_payload["data"]["section_title"] == "Chapter One"
    assert repeat_payload["data"]["synthesis"]["provider_name"] == "fake-tts"

    next_result = runner.invoke(app, ["next-chapter", "--json"], env=env)
    assert next_result.exit_code == 0
    next_payload = json.loads(next_result.stdout)
    assert next_payload["data"]["session"]["position"]["section_index"] == 1

    pause_result = runner.invoke(app, ["pause", "--json"], env=env)
    assert pause_result.exit_code == 0

    resume_result = runner.invoke(app, ["resume", "--json"], env=env)
    assert resume_result.exit_code == 0
    resume_payload = json.loads(resume_result.stdout)
    assert resume_payload["data"]["session"]["state"] == "READING"


def test_listen_command_dispatches_fake_voice_command(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "listen.sqlite3"),
        "MARGINALIA_FAKE_COMMANDS": "pausa",
    }

    ingest_result = runner.invoke(app, ["ingest", str(source_path), "--json"], env=env)
    document_id = json.loads(ingest_result.stdout)["data"]["document"]["document_id"]
    assert runner.invoke(app, ["play", document_id, "--json"], env=env).exit_code == 0

    result = runner.invoke(app, ["listen", "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["handled_command"] == "pause"
    assert payload["data"]["command_result"]["data"]["session"]["state"] == "PAUSED"


def test_control_loop_processes_multiple_fake_commands(tmp_path: Path) -> None:
    runner = CliRunner()
    source_path = Path("tests/fixtures/sample_document.txt").resolve()
    env = {
        "MARGINALIA_DB_PATH": str(tmp_path / "control-loop.sqlite3"),
        "MARGINALIA_FAKE_COMMANDS": "pausa,continua",
    }

    ingest_result = runner.invoke(app, ["ingest", str(source_path), "--json"], env=env)
    document_id = json.loads(ingest_result.stdout)["data"]["document"]["document_id"]
    assert runner.invoke(app, ["play", document_id, "--json"], env=env).exit_code == 0

    result = runner.invoke(app, ["control-loop", "--max-commands", "2", "--json"], env=env)

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert payload["data"]["handled_count"] == 2
    assert payload["data"]["iterations"][0]["data"]["handled_command"] == "pause"
    assert payload["data"]["iterations"][1]["data"]["handled_command"] == "resume"
