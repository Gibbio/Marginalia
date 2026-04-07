"""Long-running runtime manager owned by the backend process."""

from __future__ import annotations

import threading
import time

from marginalia_backend.bootstrap import BackendContainer
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.application.services.runtime_loop import RuntimeLoop, StepStatus


class BackendRuntimeManager:
    """Manage the lifetime of the active read-while-listen runtime."""

    def __init__(self, container: BackendContainer) -> None:
        self._container = container
        self._loop: RuntimeLoop | None = None
        self._thread: threading.Thread | None = None
        self._lock = threading.Lock()

    def start_session(self, target: str | None) -> OperationResult:
        """Start a background reading runtime."""

        with self._lock:
            if self._thread is not None and self._thread.is_alive():
                return OperationResult.error("A reading session is already active.")

            loop = self._container.reading_runtime_service.create_loop()
            start_result = loop.start(target)
            if start_result.status is OperationStatus.ERROR:
                return start_result

            self._loop = loop
            thread = threading.Thread(target=self._run_loop, daemon=True)
            thread.start()
            self._thread = thread
            return OperationResult.ok("Reading session started.", data=start_result.data)

    def stop_session(self) -> OperationResult:
        """Request stop for the active runtime or persisted session."""

        with self._lock:
            loop = self._loop
            thread = self._thread
        if loop is not None:
            loop.request_shutdown()
            if thread is not None:
                thread.join(timeout=5.0)
            return OperationResult.ok("Stop requested for the active runtime.")
        return self._container.reading_runtime_service.stop()

    def _run_loop(self) -> None:
        loop = self._loop
        if loop is None:
            return

        try:
            with loop:
                while loop.step() is StepStatus.CONTINUE:
                    time.sleep(0.05)
        finally:
            loop.finalize()
            with self._lock:
                self._loop = None
                self._thread = None
