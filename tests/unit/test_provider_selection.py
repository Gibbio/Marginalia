"""Provider selection and fallback tests."""

from __future__ import annotations

from pathlib import Path

from pytest import MonkeyPatch

from marginalia_cli.bootstrap import build_container


def test_container_falls_back_to_fake_real_providers_when_not_ready(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "fallback.sqlite3"))
    monkeypatch.setenv("MARGINALIA_COMMAND_STT_PROVIDER", "vosk")
    monkeypatch.setenv("MARGINALIA_TTS_PROVIDER", "kokoro")
    monkeypatch.setenv("MARGINALIA_KOKORO_PYTHON_EXECUTABLE", "missing-kokoro-python")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_PROVIDER", "subprocess")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_COMMAND", "missing-playback-command")

    container = build_container(verbose=False, config_path=None)

    assert container.command_stt.describe_capabilities().provider_name == "fake-command-stt"
    assert container.speech_synthesizer.describe_capabilities().provider_name == "fake-tts"
    assert container.playback_engine.describe_capabilities().provider_name == "fake-playback"


def test_container_can_select_unready_real_providers_when_fallback_disabled(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "providers.sqlite3"))
    monkeypatch.setenv("MARGINALIA_COMMAND_STT_PROVIDER", "vosk")
    monkeypatch.setenv("MARGINALIA_TTS_PROVIDER", "kokoro")
    monkeypatch.setenv("MARGINALIA_KOKORO_PYTHON_EXECUTABLE", "missing-kokoro-python")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_PROVIDER", "subprocess")
    monkeypatch.setenv("MARGINALIA_ALLOW_PROVIDER_FALLBACK", "false")

    container = build_container()

    assert container.command_stt.describe_capabilities().provider_name == "vosk-command-stt"
    assert container.speech_synthesizer.describe_capabilities().provider_name == "kokoro"
    assert container.playback_engine.describe_capabilities().provider_name == "subprocess-playback"
