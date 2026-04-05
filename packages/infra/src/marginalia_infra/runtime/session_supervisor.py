"""File-backed runtime supervision for the single foreground read loop."""

from __future__ import annotations

import json
import os
import signal
import subprocess
import time
from dataclasses import asdict
from datetime import datetime
from pathlib import Path

from marginalia_core.ports.runtime import (
    RuntimeCleanupReport,
    RuntimeSessionRecord,
)


class FileRuntimeSupervisor:
    """Persist and clean up the single active Marginalia runtime."""

    def __init__(self, runtime_file: Path) -> None:
        self._runtime_file = runtime_file

    def activate(self, record: RuntimeSessionRecord) -> None:
        self._runtime_file.parent.mkdir(parents=True, exist_ok=True)
        payload = asdict(record)
        payload["started_at"] = record.started_at.isoformat()
        payload["working_directory"] = (
            str(record.working_directory) if record.working_directory is not None else None
        )
        self._runtime_file.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    def current_runtime(self) -> RuntimeSessionRecord | None:
        if not self._runtime_file.exists():
            return None
        try:
            payload = json.loads(self._runtime_file.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None
        try:
            started_at = datetime.fromisoformat(str(payload["started_at"]))
            working_directory = payload.get("working_directory")
            return RuntimeSessionRecord(
                process_id=int(payload["process_id"]),
                session_id=str(payload["session_id"]),
                document_id=str(payload["document_id"]),
                command_language=str(payload["command_language"]),
                started_at=started_at,
                entrypoint=str(payload.get("entrypoint", "play")),
                working_directory=(
                    Path(str(working_directory)) if working_directory not in {None, ""} else None
                ),
            )
        except (KeyError, TypeError, ValueError):
            return None

    def cleanup_existing_runtime(self, *, current_process_id: int) -> RuntimeCleanupReport:
        record = self.current_runtime()
        if record is None:
            self.clear()
            return RuntimeCleanupReport(runtime_found=False, record_removed=False)

        notes: list[str] = []
        terminated_process_ids: list[int] = []
        record_removed = False
        if record.process_id == current_process_id:
            self.clear(process_id=record.process_id)
            return RuntimeCleanupReport(
                runtime_found=True,
                record_removed=True,
                notes=("Removed a stale self-referential runtime record.",),
            )

        if not _process_exists(record.process_id):
            notes.append("Removed a stale runtime record for a process that is no longer alive.")
            self.clear(process_id=record.process_id)
            return RuntimeCleanupReport(
                runtime_found=True,
                record_removed=True,
                notes=tuple(notes),
            )

        command_line = _process_command_line(record.process_id)
        if command_line and "marginalia" not in command_line.lower():
            notes.append(
                "Skipped terminating the recorded pid because it does not look like Marginalia."
            )
        else:
            if _terminate_process(record.process_id):
                terminated_process_ids.append(record.process_id)
                notes.append(f"Terminated stale runtime pid {record.process_id}.")
            else:
                notes.append(
                    "Unable to terminate stale runtime pid "
                    f"{record.process_id}; manual cleanup may be required."
                )

        self.clear(process_id=record.process_id)
        record_removed = True
        return RuntimeCleanupReport(
            runtime_found=True,
            record_removed=record_removed,
            terminated_process_ids=tuple(terminated_process_ids),
            notes=tuple(notes),
        )

    def clear(self, *, process_id: int | None = None) -> None:
        record = self.current_runtime()
        if record is not None and process_id is not None and record.process_id != process_id:
            return
        try:
            self._runtime_file.unlink()
        except FileNotFoundError:
            return


def _process_exists(process_id: int) -> bool:
    process_state = _process_state(process_id)
    if process_state is not None:
        return bool(process_state) and not process_state.startswith("Z")
    try:
        os.kill(process_id, 0)
    except OSError:
        return False
    return True


def _terminate_process(process_id: int) -> bool:
    try:
        os.kill(process_id, signal.SIGTERM)
    except OSError:
        return False

    for _ in range(20):
        if not _process_exists(process_id):
            return True
        time.sleep(0.05)

    try:
        os.kill(process_id, signal.SIGKILL)
    except OSError:
        return not _process_exists(process_id)
    return not _process_exists(process_id)


def _process_command_line(process_id: int) -> str | None:
    try:
        completed = subprocess.run(
            ["ps", "-p", str(process_id), "-o", "command="],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    command_line = completed.stdout.strip()
    return command_line or None


def _process_state(process_id: int) -> str | None:
    try:
        completed = subprocess.run(
            ["ps", "-p", str(process_id), "-o", "stat="],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    state = completed.stdout.strip()
    return state or None
