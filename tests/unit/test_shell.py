"""Tests for the interactive Marginalia shell."""

from __future__ import annotations

import io
from unittest.mock import MagicMock

from marginalia_cli.shell import MarginaliaShell
from marginalia_core.application.result import OperationResult, OperationStatus


def _make_container() -> MagicMock:
    """Build a mock CliContainer with enough surface for shell tests."""

    container = MagicMock()
    container.settings.doctor_report.return_value = {
        "provider_checks": {
            "kokoro": {"ready": True},
            "vosk": {"ready": False},
            "playback": {"ready": True},
        }
    }
    return container


def _run_shell(container: MagicMock, commands: str) -> str:
    """Run the shell with scripted input and capture stdout."""

    shell = MarginaliaShell(container)
    shell.prompt = ""
    shell.intro = ""
    captured = io.StringIO()

    import sys

    old_stdout = sys.stdout
    sys.stdout = captured
    try:
        shell.cmdqueue = commands.strip().splitlines()
        shell.cmdloop()
    except SystemExit:
        pass
    finally:
        sys.stdout = old_stdout

    return captured.getvalue()


def test_quit_exits_cleanly() -> None:
    """The quit command produces 'Bye.' and exits."""

    output = _run_shell(_make_container(), "quit")
    assert "Bye." in output


def test_exit_exits_cleanly() -> None:
    """The exit command also works."""

    output = _run_shell(_make_container(), "exit")
    assert "Bye." in output


def test_unknown_command_shows_error() -> None:
    """Typing a nonsense command produces an error message."""

    output = _run_shell(_make_container(), "foobar\nquit")
    assert "[error]" in output
    assert "foobar" in output


def test_status_no_session() -> None:
    """Status when there is no active session shows a helpful message."""

    container = _make_container()
    container.session_query_service.current_status.return_value = OperationResult.ok(
        "No active session.",
        data={"session": None},
    )
    output = _run_shell(container, "status\nquit")
    assert "No active session" in output


def test_pause_delegates_to_reader_service() -> None:
    """The pause command calls reader_service.pause()."""

    container = _make_container()
    container.reader_service.pause.return_value = OperationResult.ok("Paused.")
    container.session_query_service.current_status.return_value = OperationResult(
        status=OperationStatus.ERROR, message="no session", data={}
    )
    output = _run_shell(container, "pause\nquit")
    container.reader_service.pause.assert_called_once_with(command_source="shell")
    assert "Paused." in output


def test_doctor_shows_provider_readiness() -> None:
    """The doctor command displays provider status."""

    container = _make_container()
    output = _run_shell(container, "doctor\nquit")
    assert "[+] kokoro" in output
    assert "[-] vosk" in output


def test_ingest_requires_path() -> None:
    """Ingest with no argument prints a usage error."""

    output = _run_shell(_make_container(), "ingest\nquit")
    assert "[error]" in output
    assert "Usage" in output


def test_ingest_nonexistent_file() -> None:
    """Ingest with a nonexistent path prints an error."""

    output = _run_shell(_make_container(), "ingest /nonexistent/path.txt\nquit")
    assert "[error]" in output
    assert "not found" in output.lower()
