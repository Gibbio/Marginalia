"""Tests for whisper.cpp dictation transcriber integration."""

from __future__ import annotations

from pathlib import Path

from pytest import MonkeyPatch

from marginalia_adapters.real.whisper_cpp import (
    WHISPER_CPP_CAPABILITIES,
    WhisperCppDictationTranscriber,
)
from marginalia_cli.bootstrap import build_container
from marginalia_infra.config.settings import AppSettings


def test_whisper_cpp_capabilities_report_correct_interface() -> None:
    """The capabilities report identifies the provider as dictation-stt."""

    assert WHISPER_CPP_CAPABILITIES.provider_name == "whisper-cpp"
    assert WHISPER_CPP_CAPABILITIES.interface_kind == "dictation-stt"
    assert WHISPER_CPP_CAPABILITIES.offline_capable is True
    assert "it" in WHISPER_CPP_CAPABILITIES.supported_languages


def test_whisper_cpp_adapter_describes_capabilities() -> None:
    """The adapter instance returns the correct capabilities."""

    adapter = WhisperCppDictationTranscriber(
        model_path=Path("/nonexistent/model.bin"),
    )
    caps = adapter.describe_capabilities()
    assert caps.provider_name == "whisper-cpp"


def test_whisper_cpp_transcribe_raises_when_executable_missing(tmp_path: Path) -> None:
    """Transcription fails clearly when the executable is not on PATH."""

    adapter = WhisperCppDictationTranscriber(
        executable="nonexistent-whisper-binary-xyz",
        model_path=tmp_path / "model.bin",
    )
    try:
        adapter.transcribe(session_id="test", note_id="test")
        raise AssertionError("Expected RuntimeError")  # noqa: TRY301
    except RuntimeError as exc:
        assert "not available" in str(exc).lower()


def test_whisper_cpp_transcribe_raises_when_model_missing(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Transcription fails clearly when the model file does not exist."""

    # Create a dummy executable so shutil.which finds it
    dummy_exe = tmp_path / "whisper-cpp"
    dummy_exe.write_text("#!/bin/sh\nexit 0\n")
    dummy_exe.chmod(0o755)
    monkeypatch.setenv("PATH", str(tmp_path) + ":" + str(Path("/usr/bin")))

    adapter = WhisperCppDictationTranscriber(
        executable="whisper-cpp",
        model_path=tmp_path / "nonexistent-model.bin",
    )
    try:
        adapter.transcribe(session_id="test", note_id="test")
        raise AssertionError("Expected RuntimeError")  # noqa: TRY301
    except RuntimeError as exc:
        assert "does not exist" in str(exc).lower()


def test_doctor_report_includes_whisper_cpp_section() -> None:
    """The doctor report has a whisper_cpp provider check section."""

    settings = AppSettings.load(config_path=None)
    report = settings.doctor_report()

    assert "whisper_cpp" in report["provider_checks"]
    whisper = report["provider_checks"]["whisper_cpp"]
    assert "executable" in whisper
    assert "executable_available" in whisper
    assert "model_path" in whisper
    assert "model_exists" in whisper
    assert "ready" in whisper


def test_doctor_report_whisper_not_ready_without_model() -> None:
    """Without a model path, whisper-cpp reports not ready."""

    settings = AppSettings.load(config_path=None)
    report = settings.doctor_report()

    whisper = report["provider_checks"]["whisper_cpp"]
    assert whisper["model_exists"] is False
    assert whisper["ready"] is False


def test_bootstrap_falls_back_to_fake_when_whisper_not_ready(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """When whisper-cpp is requested but not ready, bootstrap falls back to fake."""

    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "whisper-fb.sqlite3"))
    monkeypatch.setenv("MARGINALIA_DICTATION_STT_PROVIDER", "whisper-cpp")
    monkeypatch.setenv("MARGINALIA_WHISPER_CPP_EXECUTABLE", "nonexistent-whisper")

    container = build_container()

    assert container.dictation_stt.describe_capabilities().provider_name == "fake-dictation-stt"


def test_bootstrap_selects_whisper_when_fallback_disabled(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """When fallback is disabled, bootstrap selects whisper-cpp even if not ready."""

    monkeypatch.setenv("MARGINALIA_DB_PATH", str(tmp_path / "whisper-nofb.sqlite3"))
    monkeypatch.setenv("MARGINALIA_DICTATION_STT_PROVIDER", "whisper-cpp")
    monkeypatch.setenv("MARGINALIA_ALLOW_PROVIDER_FALLBACK", "false")

    container = build_container()

    assert container.dictation_stt.describe_capabilities().provider_name == "whisper-cpp"


def test_settings_load_whisper_defaults() -> None:
    """Default whisper settings are sensible without any config file."""

    settings = AppSettings.load(config_path=None)

    assert settings.whisper_cpp_executable == "whisper-cpp"
    assert settings.whisper_cpp_model_path is None
    assert settings.whisper_cpp_language == "it"
    assert settings.whisper_cpp_max_record_seconds == 120
