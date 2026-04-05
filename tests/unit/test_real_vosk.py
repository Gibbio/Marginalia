"""Real Vosk adapter tests."""

from __future__ import annotations

import sys
import types
from pathlib import Path
from typing import Literal

import pytest

from marginalia_adapters.real.vosk import VoskCommandRecognizer


def test_vosk_recognizer_raises_clear_error_without_input_device(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model_path = tmp_path / "model"
    model_path.mkdir()

    fake_sounddevice = types.ModuleType("sounddevice")
    fake_sounddevice.default = types.SimpleNamespace(device=(-1, 1))  # type: ignore[attr-defined]
    fake_sounddevice.query_devices = (  # type: ignore[attr-defined]
        lambda: [{"name": "Speaker", "max_input_channels": 0}]
    )

    fake_vosk = types.ModuleType("vosk")

    class DummyModel:
        def __init__(self, model_path: str) -> None:
            self.model_path = model_path

    class DummyRecognizer:
        def __init__(self, model: object, sample_rate: int, grammar: str) -> None:
            self.model = model
            self.sample_rate = sample_rate
            self.grammar = grammar

        def AcceptWaveform(self, data: bytes) -> bool:
            return False

        def Result(self) -> str:
            return '{"text": ""}'

        def FinalResult(self) -> str:
            return '{"text": ""}'

    fake_vosk.Model = DummyModel  # type: ignore[attr-defined]
    fake_vosk.KaldiRecognizer = DummyRecognizer  # type: ignore[attr-defined]

    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)
    monkeypatch.setitem(sys.modules, "vosk", fake_vosk)

    recognizer = VoskCommandRecognizer(model_path=model_path, commands=("pausa",))

    with pytest.raises(RuntimeError, match="No input audio device is available"):
        recognizer.listen_for_command()


def test_vosk_recognizer_uses_first_input_device_when_default_is_missing(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model_path = tmp_path / "model"
    model_path.mkdir()
    captured_device: list[int] = []

    fake_sounddevice = types.ModuleType("sounddevice")
    fake_sounddevice.default = types.SimpleNamespace(device=(-1, 1))  # type: ignore[attr-defined]
    fake_sounddevice.query_devices = (  # type: ignore[attr-defined]
        lambda: [
            {"name": "Speaker", "max_input_channels": 0},
            {"name": "USB Mic", "max_input_channels": 1},
        ]
    )

    class DummyRawInputStream:
        def __init__(self, *, device: int, **_: object) -> None:
            captured_device.append(device)

        def __enter__(self) -> DummyRawInputStream:
            return self

        def __exit__(
            self,
            exc_type: object,
            exc: object,
            tb: object,
        ) -> Literal[False]:
            return False

    fake_sounddevice.RawInputStream = DummyRawInputStream  # type: ignore[attr-defined]

    fake_vosk = types.ModuleType("vosk")

    class DummyModel:
        def __init__(self, model_path: str) -> None:
            self.model_path = model_path

    class DummyRecognizer:
        def __init__(self, model: object, sample_rate: int, grammar: str) -> None:
            self.model = model
            self.sample_rate = sample_rate
            self.grammar = grammar

        def AcceptWaveform(self, data: bytes) -> bool:
            return False

        def Result(self) -> str:
            return '{"text": ""}'

        def FinalResult(self) -> str:
            return '{"text": ""}'

    fake_vosk.Model = DummyModel  # type: ignore[attr-defined]
    fake_vosk.KaldiRecognizer = DummyRecognizer  # type: ignore[attr-defined]

    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)
    monkeypatch.setitem(sys.modules, "vosk", fake_vosk)

    recognizer = VoskCommandRecognizer(
        model_path=model_path,
        commands=("pausa",),
        timeout_seconds=0.0,
    )

    assert recognizer.listen_for_command() is None
    assert captured_device == [1]
