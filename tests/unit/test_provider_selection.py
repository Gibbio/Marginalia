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


def test_fake_providers_report_distinct_capabilities(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "caps.sqlite3"))
    monkeypatch.setenv("MARGINALIA_TTS_PROVIDER", "fake")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_PROVIDER", "fake")

    container = build_container()

    stt_caps = container.command_stt.describe_capabilities()
    tts_caps = container.speech_synthesizer.describe_capabilities()
    playback_caps = container.playback_engine.describe_capabilities()
    assert stt_caps.interface_kind == "command-stt"
    assert tts_caps.interface_kind == "speech-synthesizer"
    assert playback_caps.interface_kind == "playback"
    assert stt_caps.offline_capable is True
    assert tts_caps.offline_capable is True
    assert playback_caps.offline_capable is True


def test_fallback_is_visible_in_runtime_details(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """When a real provider is requested but unavailable, the resolved name is fake."""

    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "visible.sqlite3"))
    monkeypatch.setenv("MARGINALIA_COMMAND_STT_PROVIDER", "vosk")
    monkeypatch.setenv("MARGINALIA_TTS_PROVIDER", "kokoro")
    monkeypatch.setenv("MARGINALIA_KOKORO_PYTHON_EXECUTABLE", "missing-kokoro-python")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_PROVIDER", "subprocess")
    monkeypatch.setenv("MARGINALIA_PLAYBACK_COMMAND", "missing-playback-command")

    container = build_container()

    requested_tts = container.settings.tts_provider
    resolved_tts = container.speech_synthesizer.describe_capabilities().provider_name
    assert requested_tts == "kokoro"
    assert resolved_tts == "fake-tts"
    assert requested_tts != resolved_tts
