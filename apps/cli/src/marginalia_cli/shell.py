"""Interactive Marginalia shell."""

from __future__ import annotations

import cmd
import signal
import threading
from pathlib import Path
from typing import Any

from marginalia_cli.bootstrap import CliContainer
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.application.services.runtime_loop import RuntimeLoop, StepStatus


class MarginaliaShell(cmd.Cmd):
    """Interactive REPL for Marginalia."""

    intro = (
        "\n  Marginalia interactive shell\n"
        "  Type 'help' for available commands, 'quit' to exit.\n"
    )
    prompt = "marginalia> "

    def __init__(self, container: CliContainer) -> None:
        super().__init__()
        self._container = container
        self._loop: RuntimeLoop | None = None
        self._loop_thread: threading.Thread | None = None
        self._loop_lock = threading.Lock()

    # ------------------------------------------------------------------
    # Playback
    # ------------------------------------------------------------------

    def do_play(self, arg: str) -> None:
        """Start reading a document. Usage: play [file-or-id]"""

        if self._loop_thread is not None and self._loop_thread.is_alive():
            self._print_error("A reading session is already active. Use 'stop' first.")
            return

        target = arg.strip() or None
        loop = self._container.reading_runtime_service.create_loop()
        start_result = loop.start(target)
        if start_result.status is OperationStatus.ERROR:
            self._print_error(start_result.message)
            return

        self._loop = loop
        doc_title = start_result.data.get("target", {}).get("document_id", "unknown")
        session = start_result.data.get("session")
        if session is not None:
            self._print_ok(f"Reading started: {session.document_id}")
        else:
            self._print_ok(f"Reading started: {doc_title}")

        self._print_status_line()

        thread = threading.Thread(target=self._run_loop, daemon=True)
        thread.start()
        self._loop_thread = thread

    def do_pause(self, arg: str) -> None:
        """Pause playback."""

        result = self._container.reader_service.pause(command_source="shell")
        self._print_result(result)

    def do_resume(self, arg: str) -> None:
        """Resume playback."""

        result = self._container.reader_service.resume(command_source="shell")
        self._print_result(result)

    def do_stop(self, arg: str) -> None:
        """Stop the reading session."""

        if self._loop is not None:
            self._loop.request_shutdown()
            if self._loop_thread is not None:
                self._loop_thread.join(timeout=5.0)
            self._loop = None
            self._loop_thread = None
            self._print_ok("Stopped.")
        else:
            result = self._container.reader_service.stop(command_source="shell")
            self._print_result(result)

    def do_repeat(self, arg: str) -> None:
        """Repeat the current chunk."""

        result = self._container.reader_service.repeat_current_chunk(command_source="shell")
        self._print_result(result)

    def do_rewind(self, arg: str) -> None:
        """Go back one chunk."""

        result = self._container.reader_service.previous_chunk(command_source="shell")
        self._print_result(result)

    def do_next(self, arg: str) -> None:
        """Skip to the next chapter."""

        result = self._container.reader_service.next_chapter(command_source="shell")
        self._print_result(result)

    def do_restart(self, arg: str) -> None:
        """Restart the current chapter."""

        result = self._container.reader_service.restart_chapter(command_source="shell")
        self._print_result(result)

    # ------------------------------------------------------------------
    # Status and inspection
    # ------------------------------------------------------------------

    def do_status(self, arg: str) -> None:
        """Show current session state and reading progress."""

        result = self._container.session_query_service.current_status()
        if result.status is OperationStatus.ERROR:
            self._print_error(result.message)
            return

        data = result.data
        if data.get("session") is None:
            self._print_info("No active session. Use 'play <file>' to start.")
            return

        self._print_status_from_data(data)

    def do_documents(self, arg: str) -> None:
        """List ingested documents."""

        documents = self._container.ingestion_service._document_repository.list_documents()
        if not documents:
            self._print_info("No documents ingested yet.")
            return
        for doc in documents:
            chunks = doc.total_chunk_count
            chapters = doc.chapter_count
            print(f"  {doc.document_id}  {doc.title} ({chapters} ch, {chunks} chunks)")

    def do_notes(self, arg: str) -> None:
        """List notes for the active session's document."""

        session = self._container.reader_service._session_repository.get_active_session()
        if session is None:
            self._print_info("No active session.")
            return
        notes = list(
            self._container.note_service._note_repository.list_notes_for_document(
                session.document_id
            )
        )
        if not notes:
            self._print_info("No notes yet.")
            return
        for note in notes:
            pos = f"s{note.position.section_index}:c{note.position.chunk_index}"
            excerpt = note.transcript[:60] if note.transcript else "(empty)"
            print(f"  [{pos}] {excerpt}")

    def do_ingest(self, arg: str) -> None:
        """Ingest a document without playing it. Usage: ingest <file>"""

        path = arg.strip()
        if not path:
            self._print_error("Usage: ingest <file>")
            return
        resolved = Path(path).expanduser()
        if not resolved.exists():
            self._print_error(f"File not found: {path}")
            return
        result = self._container.ingestion_service.ingest_text_file(resolved)
        self._print_result(result)

    # ------------------------------------------------------------------
    # Notes
    # ------------------------------------------------------------------

    def do_note(self, arg: str) -> None:
        """Capture a note. Usage: note [text] — if no text, uses dictation."""

        text = arg.strip() or None
        start_result = self._container.note_service.start_note_capture()
        if start_result.status is OperationStatus.ERROR:
            self._print_error(start_result.message)
            return
        stop_result = self._container.note_service.stop_note_capture(transcript=text)
        self._print_result(stop_result)

    # ------------------------------------------------------------------
    # Doctor
    # ------------------------------------------------------------------

    def do_doctor(self, arg: str) -> None:
        """Show provider readiness."""

        checks = self._container.settings.doctor_report()["provider_checks"]
        for name, info in checks.items():
            ready = info.get("ready", False)
            status_char = "+" if ready else "-"
            print(f"  [{status_char}] {name}: {'ready' if ready else 'not ready'}")

    # ------------------------------------------------------------------
    # Shell lifecycle
    # ------------------------------------------------------------------

    def do_quit(self, arg: str) -> bool:
        """Exit the shell."""

        self._cleanup()
        print("  Bye.")
        return True

    def do_exit(self, arg: str) -> bool:
        """Exit the shell."""

        return self.do_quit(arg)

    do_EOF = do_quit  # Ctrl-D

    def emptyline(self) -> bool:
        """Do nothing on empty input."""

        return False

    def default(self, line: str) -> None:
        """Handle unknown commands."""

        self._print_error(f"Unknown command: {line.split()[0]}. Type 'help' for commands.")

    # ------------------------------------------------------------------
    # Background loop
    # ------------------------------------------------------------------

    def _run_loop(self) -> None:
        """Run the RuntimeLoop in the background thread."""

        loop = self._loop
        if loop is None:
            return

        prev_sigint = signal.getsignal(signal.SIGINT)

        try:
            with loop:
                while loop.step() is StepStatus.CONTINUE:
                    pass
        except Exception:
            pass
        finally:
            signal.signal(signal.SIGINT, prev_sigint)

        result = loop.finalize()
        outcome = "completed" if result.data and result.data.get("runtime", {}).get(
            "outcome") == "completed" else "stopped"
        print(f"\n  [{outcome}] Reading session ended.")
        print(self.prompt, end="", flush=True)

    def _cleanup(self) -> None:
        """Stop any active session before exiting."""

        if self._loop is not None:
            self._loop.request_shutdown()
            if self._loop_thread is not None:
                self._loop_thread.join(timeout=5.0)
            self._loop = None
            self._loop_thread = None

    # ------------------------------------------------------------------
    # Output helpers
    # ------------------------------------------------------------------

    def _print_ok(self, message: str) -> None:
        print(f"  {message}")

    def _print_error(self, message: str) -> None:
        print(f"  [error] {message}")

    def _print_info(self, message: str) -> None:
        print(f"  {message}")

    def _print_result(self, result: OperationResult) -> None:
        if result.status is OperationStatus.ERROR:
            self._print_error(result.message)
        else:
            self._print_ok(result.message)
            self._print_status_line()

    def _print_status_line(self) -> None:
        """Print a compact one-line progress status."""

        result = self._container.session_query_service.current_status()
        if result.status is OperationStatus.ERROR or result.data.get("session") is None:
            return
        self._print_status_from_data(result.data)

    def _print_status_from_data(self, data: dict[str, Any]) -> None:
        progress = data.get("progress", {})
        si = progress.get("section_index", 0)
        sc = progress.get("section_count", 0)
        ci = progress.get("chunk_index", 0)
        scc = progress.get("section_chunk_count", 0)
        section_title = data.get("position", {}).get("section_title", "")
        chunk_text = data.get("position", {}).get("chunk_text", "")
        excerpt = chunk_text[:70] + "..." if len(chunk_text) > 70 else chunk_text

        session = data.get("session")
        state = ""
        if session is not None:
            raw_state = getattr(session, "state", None)
            if raw_state is not None:
                state = raw_state.value if hasattr(raw_state, "value") else str(raw_state)

        print(f"  Chapter {si + 1}/{sc}, chunk {ci + 1}/{scc} — {section_title}")
        if excerpt:
            print(f"  \"{excerpt}\"")
        if state:
            print(f"  [{state}]")
