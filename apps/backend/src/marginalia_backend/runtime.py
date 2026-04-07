"""Long-running runtime manager owned by the backend process."""

from __future__ import annotations

import logging
import threading
import time
from time import perf_counter

from marginalia_backend.bootstrap import BackendContainer
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.application.services.runtime_loop import RuntimeLoop, StepStatus

logger = logging.getLogger(__name__)


class BackendRuntimeManager:
    """Manage the lifetime of the active read-while-listen runtime."""

    def __init__(self, container: BackendContainer) -> None:
        self._container = container
        self._loop: RuntimeLoop | None = None
        self._thread: threading.Thread | None = None
        self._lock = threading.Lock()

    def start_session(self, target: str | None) -> OperationResult:
        """Start a background reading runtime."""

        started_at = perf_counter()
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
            session = start_result.data.get("session")
            resolved_target = start_result.data.get("target", {})
            logger.info(
                "timing runtime_start target=%s session=%s document=%s ingested_now=%s total_ms=%.2f",
                target or "-",
                getattr(session, "session_id", "-"),
                getattr(session, "document_id", resolved_target.get("document_id", "-")),
                resolved_target.get("ingested_now", False),
                (perf_counter() - started_at) * 1000,
            )
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
