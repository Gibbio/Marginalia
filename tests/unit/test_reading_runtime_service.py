"""Single-mode reading runtime tests."""

from __future__ import annotations

import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Literal

from marginalia_adapters.fake.playback import FakePlaybackEngine
from marginalia_adapters.fake.stt import FakeCommandRecognizer
from marginalia_adapters.fake.tts import FakeSpeechSynthesizer
from marginalia_core.application.command_router import load_command_lexicon
from marginalia_core.application.services.document_ingestion_service import (
    DocumentIngestionService,
)
from marginalia_core.application.services.reader_service import ReaderService
from marginalia_core.application.services.reading_runtime_service import ReadingRuntimeService
from marginalia_core.application.services.runtime_loop import StepStatus
from marginalia_core.domain.reading_session import PlaybackState, ReaderState
from marginalia_core.events.models import EventName
from marginalia_core.ports.runtime import RuntimeSessionRecord
from marginalia_core.ports.stt import SpeechInterruptCapture
from marginalia_infra.events import InMemoryEventBus
from marginalia_infra.runtime.session_supervisor import FileRuntimeSupervisor
from marginalia_infra.storage.sqlite import (
    SQLiteDatabase,
    SQLiteDocumentRepository,
    SQLiteSessionRepository,
)


def test_runtime_service_completes_document_and_marks_session_cleanly(
    tmp_path: Path,
) -> None:
    runtime_service, session_repository, _, event_bus = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["outcome"] == "completed"
    assert result.data["runtime"]["handled_command_count"] == 0
    restored = session_repository.get_active_session()
    assert restored is not None
    assert restored.state is ReaderState.IDLE
    assert restored.playback_state is PlaybackState.STOPPED
    assert restored.command_listening_active is False
    assert restored.command_language == "it"
    assert restored.runtime_status == "completed"
    assert any(event.name is EventName.READING_STARTED for event in event_bus.published_events)


def test_runtime_service_dispatches_commands_while_reading(tmp_path: Path) -> None:
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        commands=("pausa", "continua", "stop"),
        playback_auto_complete_after_snapshots=2,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["outcome"] == "stopped"
    assert result.data["runtime"]["handled_command_count"] == 3
    assert result.data["runtime"]["handled_commands"][0]["handled_command"] == "pause"
    assert result.data["runtime"]["handled_commands"][1]["handled_command"] == "resume"
    assert result.data["runtime"]["handled_commands"][2]["handled_command"] == "stop"
    restored = session_repository.get_active_session()
    assert restored is not None
    assert restored.state is ReaderState.IDLE
    assert restored.runtime_status == "stopped"
    assert restored.last_command == "stop"


def test_runtime_service_marks_listening_active_before_capture(tmp_path: Path) -> None:
    recognizer = _ObservingCommandRecognizer()
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        command_recognizer=recognizer,
        playback_auto_complete_after_snapshots=2,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert recognizer.observed_command_listening == [True]


def test_runtime_service_cleans_stale_runtime_record_before_start(tmp_path: Path) -> None:
    runtime_service, _, runtime_supervisor, _ = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )
    process = subprocess.Popen([sys.executable, "-c", "print('done')"])
    process.wait(timeout=5)
    runtime_supervisor.activate(_runtime_record(process.pid))

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))
    cleanup = result.data["runtime"]["cleanup"]

    assert result.status.value == "ok"
    assert cleanup["cleaned_up"] is True
    assert cleanup["runtime_report"].runtime_found is True
    assert cleanup["runtime_report"].record_removed is True


def test_step_driven_loop_completes_document(tmp_path: Path) -> None:
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )
    loop = runtime_service.create_loop()
    start_result = loop.start(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert start_result.status.value == "ok"

    steps = 0
    with loop:
        while loop.step() is StepStatus.CONTINUE:
            steps += 1
            assert steps < 200, "loop did not terminate"

    result = loop.finalize()
    assert result.status.value == "ok"
    assert result.data["runtime"]["outcome"] == "completed"


def test_step_driven_loop_stops_on_shutdown_request(tmp_path: Path) -> None:
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=100,
    )
    loop = runtime_service.create_loop()
    loop.start(str(Path("tests/fixtures/sample_document.txt").resolve()))

    with loop:
        loop.step()
        loop.request_shutdown()
        status = loop.step()

    assert status is StepStatus.STOPPED
    result = loop.finalize()
    assert result.data["runtime"]["outcome"] == "stopped"


def test_runtime_service_completion_is_distinguishable_from_stop(tmp_path: Path) -> None:
    """Completed sessions have runtime_status='completed' and last_command='document-complete'."""

    runtime_service, session_repository, _, event_bus = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["outcome"] == "completed"
    session = session_repository.get_active_session()
    assert session is not None
    assert session.runtime_status == "completed"
    assert session.last_command == "document-complete"
    assert session.last_command_source == "runtime"
    assert any(
        event.name.value == "reading.completed" for event in event_bus.published_events
    )


def test_runtime_service_stop_is_distinguishable_from_completion(tmp_path: Path) -> None:
    """Stopped sessions have runtime_status='stopped' and last_command='stop'."""

    runtime_service, session_repository, _, event_bus = _build_runtime_services(
        tmp_path,
        commands=("stop",),
        playback_auto_complete_after_snapshots=2,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["outcome"] == "stopped"
    session = session_repository.get_active_session()
    assert session is not None
    assert session.runtime_status == "stopped"
    assert session.last_command == "stop"
    assert not any(
        event.name.value == "reading.completed" for event in event_bus.published_events
    )


def test_runtime_service_dispatches_help_intent(tmp_path: Path) -> None:
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        commands=("aiuto", "stop"),
        playback_auto_complete_after_snapshots=2,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["handled_command_count"] == 2
    assert result.data["runtime"]["handled_commands"][0]["handled_command"] == "help"
    assert result.data["runtime"]["handled_commands"][1]["handled_command"] == "stop"


def test_runtime_service_dispatches_stop_alias(tmp_path: Path) -> None:
    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        commands=("fermati",),
        playback_auto_complete_after_snapshots=2,
    )

    result = runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    assert result.status.value == "ok"
    assert result.data["runtime"]["handled_commands"][0]["handled_command"] == "stop"
    session = session_repository.get_active_session()
    assert session is not None
    assert session.last_command == "stop"


def test_runtime_service_restart_after_completed_session(tmp_path: Path) -> None:
    """A second play() after document completion starts a new clean session."""

    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )
    fixture = str(Path("tests/fixtures/sample_document.txt").resolve())

    first_result = runtime_service.play(fixture)
    assert first_result.data["runtime"]["outcome"] == "completed"

    second_result = runtime_service.play(fixture)
    assert second_result.data["runtime"]["outcome"] == "completed"
    second_session = session_repository.get_active_session()
    assert second_session is not None
    assert second_session.runtime_status == "completed"
    assert second_session.startup_cleanup_summary is not None
    assert (
        "Stopped the previously persisted reading session"
        in second_session.startup_cleanup_summary
    )


def test_runtime_supervisor_skips_termination_on_pid_reuse(tmp_path: Path) -> None:
    """When the recorded PID's start time mismatches, cleanup removes the record without killing."""

    runtime_supervisor = FileRuntimeSupervisor(tmp_path / "runtime" / "active-session.json")
    import os

    record = RuntimeSessionRecord(
        process_id=os.getpid(),
        session_id="old-session",
        document_id="old-doc",
        command_language="it",
        process_start_time="FAKE_START_TIME_THAT_WILL_NOT_MATCH",
    )
    runtime_supervisor.activate(record)

    report = runtime_supervisor.cleanup_existing_runtime(current_process_id=os.getpid() + 99999)

    assert report.runtime_found is True
    assert report.record_removed is True
    assert len(report.terminated_process_ids) == 0
    assert any("start time" in note for note in report.notes)


def test_runtime_supervisor_file_locking_does_not_deadlock(tmp_path: Path) -> None:
    """Sequential activate/cleanup/clear calls complete without deadlock."""

    runtime_supervisor = FileRuntimeSupervisor(tmp_path / "runtime" / "active-session.json")
    import os

    record = RuntimeSessionRecord(
        process_id=os.getpid(),
        session_id="lock-test",
        document_id="doc-1",
        command_language="it",
    )
    runtime_supervisor.activate(record)
    current = runtime_supervisor.current_runtime()
    assert current is not None
    assert current.session_id == "lock-test"

    runtime_supervisor.clear(process_id=os.getpid())
    assert runtime_supervisor.current_runtime() is None


def test_handled_commands_capped_at_1000(tmp_path: Path) -> None:
    """The _handled_commands list does not grow beyond 1000 entries."""

    runtime_service, _, _, _ = _build_runtime_services(
        tmp_path,
        commands=tuple(["pausa", "continua"] * 600),
        playback_auto_complete_after_snapshots=9999,
    )
    loop = runtime_service.create_loop()
    start_result = loop.start(str(Path("tests/fixtures/sample_document.txt").resolve()))
    assert start_result.status.value == "ok"

    with loop:
        steps = 0
        while loop.step() is StepStatus.CONTINUE:
            steps += 1
            if steps > 1500:
                loop.request_shutdown()
                break

    assert len(loop._handled_commands) <= 1000


def test_status_truthfulness_after_completion(tmp_path: Path) -> None:
    """Status correctly reports IDLE/stopped/completed after document finishes."""

    runtime_service, session_repository, _, _ = _build_runtime_services(
        tmp_path,
        playback_auto_complete_after_snapshots=0,
    )
    runtime_service.play(str(Path("tests/fixtures/sample_document.txt").resolve()))

    session = session_repository.get_active_session()
    assert session is not None
    assert session.state is ReaderState.IDLE
    assert session.playback_state is PlaybackState.STOPPED
    assert session.runtime_status == "completed"
    assert session.command_listening_active is False
    assert session.runtime_process_id is None
    assert session.runtime_error is None


def _build_runtime_services(
    tmp_path: Path,
    *,
    commands: tuple[str, ...] = (),
    playback_auto_complete_after_snapshots: int | None = None,
    command_recognizer: FakeCommandRecognizer | None = None,
) -> tuple[ReadingRuntimeService, SQLiteSessionRepository, FileRuntimeSupervisor, InMemoryEventBus]:
    database = SQLiteDatabase(tmp_path / "marginalia.sqlite3")
    database.initialize()
    document_repository = SQLiteDocumentRepository(database)
    session_repository = SQLiteSessionRepository(database)
    event_bus = InMemoryEventBus()
    ingestion_service = DocumentIngestionService(
        document_repository=document_repository,
        event_publisher=event_bus,
    )
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )
    recognizer = command_recognizer or FakeCommandRecognizer(commands=commands)
    if isinstance(recognizer, _ObservingCommandRecognizer):
        recognizer._session_repository = session_repository
    reader_service = ReaderService(
        document_repository=document_repository,
        session_repository=session_repository,
        playback_engine=FakePlaybackEngine(
            auto_complete_after_snapshots=playback_auto_complete_after_snapshots
        ),
        speech_synthesizer=FakeSpeechSynthesizer(),
        event_publisher=event_bus,
        command_recognizer=recognizer,
        command_lexicon=lexicon,
        default_voice="if_sara",
    )
    runtime_supervisor = FileRuntimeSupervisor(tmp_path / "runtime" / "active-session.json")
    runtime_service = ReadingRuntimeService(
        document_repository=document_repository,
        session_repository=session_repository,
        ingestion_service=ingestion_service,
        reader_service=reader_service,
        command_recognizer=recognizer,
        runtime_supervisor=runtime_supervisor,
        command_lexicon=lexicon,
    )
    return runtime_service, session_repository, runtime_supervisor, event_bus


def _runtime_record(process_id: int) -> RuntimeSessionRecord:
    return RuntimeSessionRecord(
        process_id=process_id,
        session_id="stale-session",
        document_id="stale-doc",
        command_language="it",
    )


class _ObservingCommandRecognizer(FakeCommandRecognizer):
    def __init__(self, session_repository: SQLiteSessionRepository | None = None) -> None:
        super().__init__(commands=())
        self._session_repository = session_repository
        self.observed_command_listening: list[bool] = []

    def open_interrupt_monitor(self) -> _ObservingInterruptMonitor:
        return _ObservingInterruptMonitor(self)


class _ObservingInterruptMonitor:
    def __init__(self, recognizer: _ObservingCommandRecognizer) -> None:
        self._recognizer = recognizer
        self._returned = False

    def __enter__(self) -> _ObservingInterruptMonitor:
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> Literal[False]:
        return False

    def capture_next_interrupt(
        self,
        *,
        timeout_seconds: float | None = None,
        on_speech_start: Callable[[int], None] | None = None,
    ) -> SpeechInterruptCapture:
        del timeout_seconds, on_speech_start
        assert self._recognizer._session_repository is not None
        session = self._recognizer._session_repository.get_active_session()
        assert session is not None
        self._recognizer.observed_command_listening.append(session.command_listening_active)
        if self._returned:
            return SpeechInterruptCapture(
                provider_name="fake-command-stt",
                speech_detected=False,
                capture_ended_ms=100,
                timed_out=True,
            )
        self._returned = True
        return SpeechInterruptCapture(
            provider_name="fake-command-stt",
            speech_detected=True,
            speech_detected_ms=50,
            capture_started_ms=50,
            capture_ended_ms=120,
            recognized_command="stop",
            raw_text="stop",
        )

    def close(self) -> None:
        return None
