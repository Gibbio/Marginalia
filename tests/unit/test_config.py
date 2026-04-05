"""Configuration loading tests."""

from __future__ import annotations

import importlib.util
import sys
import types
from pathlib import Path

from pytest import MonkeyPatch

from marginalia_infra.config.settings import AppSettings


def test_settings_respect_environment_paths(tmp_path: Path, monkeypatch: MonkeyPatch) -> None:
    home_path = tmp_path / "home"
    database_path = tmp_path / "custom.sqlite3"
    monkeypatch.setenv("MARGINALIA_HOME", str(home_path))
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(database_path))

    settings = AppSettings.load()

    assert settings.home_dir == home_path
    assert settings.database_path == database_path
    assert settings.data_dir == home_path / "data"
    assert settings.audio_cache_dir == home_path / "data" / "audio-cache"


def test_settings_default_tts_provider_is_kokoro(monkeypatch: MonkeyPatch) -> None:
    monkeypatch.delenv("MARGINALIA_TTS_PROVIDER", raising=False)

    settings = AppSettings.load()

    assert settings.tts_provider == "kokoro"


def test_settings_default_playback_provider_remains_fake(monkeypatch: MonkeyPatch) -> None:
    monkeypatch.delenv("MARGINALIA_PLAYBACK_PROVIDER", raising=False)

    settings = AppSettings.load()

    assert settings.playback_provider == "fake"


def test_doctor_report_marks_vosk_unready_without_audio_input_device(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    model_path = tmp_path / "vosk-model"
    model_path.mkdir()

    real_find_spec = importlib.util.find_spec

    def fake_find_spec(name: str) -> object | None:
        if name in {"vosk", "sounddevice"}:
            return object()
        return real_find_spec(name)

    fake_sounddevice = types.ModuleType("sounddevice")
    fake_sounddevice.default = types.SimpleNamespace(device=(-1, 1))  # type: ignore[attr-defined]
    fake_sounddevice.query_devices = (  # type: ignore[attr-defined]
        lambda: [{"name": "Speaker", "max_input_channels": 0}]
    )

    monkeypatch.setattr(importlib.util, "find_spec", fake_find_spec)
    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)
    monkeypatch.setenv("MARGINALIA_VOSK_MODEL_PATH", str(model_path))

    settings = AppSettings.load()
    report = settings.doctor_report()

    assert report["provider_checks"]["vosk"]["input_device_available"] is False
    assert report["provider_checks"]["vosk"]["input_device_count"] == 0
    assert report["provider_checks"]["vosk"]["ready"] is False


def test_doctor_report_marks_kokoro_unready_without_runtime(
    monkeypatch: MonkeyPatch,
) -> None:
    monkeypatch.setenv("MARGINALIA_KOKORO_PYTHON_EXECUTABLE", "missing-kokoro-python")

    settings = AppSettings.load()
    report = settings.doctor_report()

    assert report["provider_checks"]["kokoro"]["python_available"] is False
    assert report["provider_checks"]["kokoro"]["ready"] is False
