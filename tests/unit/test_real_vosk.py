"""Real Vosk adapter tests."""

from __future__ import annotations

import sys
import types
from pathlib import Path
from typing import Literal

import pytest

import marginalia_adapters.real.vosk as vosk_module
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
        recognizer.capture_interrupt()


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

    capture = recognizer.capture_interrupt(timeout_seconds=0.0)
    assert capture.timed_out is True
    assert captured_device == [1]


def test_vosk_capture_interrupt_reports_detection_and_selected_device(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model_path = tmp_path / "model"
    model_path.mkdir()
    captured_device: list[int] = []
    detected_offsets: list[int] = []

    fake_sounddevice = types.ModuleType("sounddevice")
    fake_sounddevice.default = types.SimpleNamespace(device=(-1, 2))  # type: ignore[attr-defined]
    fake_sounddevice.query_devices = (  # type: ignore[attr-defined]
        lambda: [
            {"name": "Speaker", "max_input_channels": 0},
            {"name": "Desk Mic", "max_input_channels": 1},
            {"name": "AirPods Pro 3", "max_input_channels": 1},
        ]
    )

    class DummyRawInputStream:
        def __init__(self, *, device: int, callback: object, **_: object) -> None:
            captured_device.append(device)
            self._callback = callback

        def __enter__(self) -> DummyRawInputStream:
            callback = self._callback
            assert callable(callback)
            callback(b"\xff\x7f" * 4000, 4000, None, None)
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
            self._accepted = False

        def AcceptWaveform(self, data: bytes) -> bool:
            del data
            if self._accepted:
                return False
            self._accepted = True
            return True

        def Result(self) -> str:
            return '{"text": "pausa"}'

        def FinalResult(self) -> str:
            return '{"text": ""}'

    fake_vosk.Model = DummyModel  # type: ignore[attr-defined]
    fake_vosk.KaldiRecognizer = DummyRecognizer  # type: ignore[attr-defined]

    monkeypatch.setitem(sys.modules, "sounddevice", fake_sounddevice)
    monkeypatch.setitem(sys.modules, "vosk", fake_vosk)
    monkeypatch.setattr(
        vosk_module._VoskSpeechInterruptMonitor,
        "_drain_audio_queue",
        lambda self: None,
    )

    recognizer = VoskCommandRecognizer(
        model_path=model_path,
        commands=("pausa",),
        input_device_name="Desk Mic",
    )

    capture = recognizer.capture_interrupt(on_speech_start=detected_offsets.append)

    assert captured_device == [1]
    assert detected_offsets and detected_offsets[0] >= 0
    assert capture.speech_detected is True
    assert capture.recognized_command == "pausa"
    assert capture.input_device_index == 1
    assert capture.input_device_name == "Desk Mic"


def test_vosk_interrupt_monitor_keeps_stream_open_across_attempts(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model_path = tmp_path / "model"
    model_path.mkdir()
    stream_enters: list[int] = []

    fake_sounddevice = types.ModuleType("sounddevice")
    fake_sounddevice.default = types.SimpleNamespace(device=(-1, 1))  # type: ignore[attr-defined]
    fake_sounddevice.query_devices = (  # type: ignore[attr-defined]
        lambda: [{"name": "Desk Mic", "max_input_channels": 1}]
    )

    class DummyRawInputStream:
        def __init__(self, **_: object) -> None:
            pass

        def __enter__(self) -> DummyRawInputStream:
            stream_enters.append(1)
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
            del data
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

    with recognizer.open_interrupt_monitor() as monitor:
        first = monitor.capture_next_interrupt(timeout_seconds=0.0)
        second = monitor.capture_next_interrupt(timeout_seconds=0.0)

    assert stream_enters == [1]
    assert first.timed_out is True
    assert second.timed_out is True
