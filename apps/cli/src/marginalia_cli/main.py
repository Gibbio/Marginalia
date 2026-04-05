"""Typer application for Marginalia."""

from __future__ import annotations

import json
import signal
from dataclasses import asdict, is_dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

import typer

from marginalia_cli.bootstrap import CliContainer, build_container
from marginalia_core.application.result import OperationResult, OperationStatus
from marginalia_core.application.services.runtime_loop import StepStatus

app = typer.Typer(
    name="marginalia",
    help="Local AI-first voice reading and annotation engine.",
    no_args_is_help=True,
    add_completion=False,
)

CONFIG_OPTION = typer.Option(
    None,
    "--config",
    help="Optional path to a TOML config file.",
    exists=False,
    dir_okay=False,
    resolve_path=True,
)
VERBOSE_OPTION = typer.Option(False, "--verbose", help="Enable verbose CLI logging.")
JSON_OPTION = typer.Option(False, "--json", help="Emit machine-readable JSON output.")
DOCUMENT_PATH_ARGUMENT = typer.Argument(..., exists=True, dir_okay=False, resolve_path=True)
PLAY_TARGET_ARGUMENT = typer.Argument(
    None,
    help="Filesystem path or stored document id. Uses the active or latest document if omitted.",
)
TOPIC_ARGUMENT = typer.Argument(..., help="Topic to summarize.")
QUERY_ARGUMENT = typer.Argument(..., help="Free-text search query.")
NOTE_TEXT_OPTION = typer.Option(
    None,
    "--text",
    help="Optional explicit note content. If omitted, the fake dictation adapter is used.",
)


def _json_default(value: object) -> object:
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, datetime):
        return value.isoformat()
    if isinstance(value, Enum):
        return value.value
    if is_dataclass(value) and not isinstance(value, type):
        return asdict(value)
    return str(value)


def _emit_result(result: OperationResult, *, as_json: bool) -> None:
    payload = result.to_dict()
    if as_json:
        typer.echo(json.dumps(payload, indent=2, default=_json_default))
        return

    typer.echo(f"[{result.status.value}] {result.message}")
    if result.data:
        typer.echo(json.dumps(result.data, indent=2, default=_json_default))


def _exit_code(result: OperationResult) -> int:
    return 1 if result.status is OperationStatus.ERROR else 0


def _container_from_context(ctx: typer.Context) -> CliContainer:
    config_path = ctx.obj.get("config_path") if ctx.obj else None
    verbose = bool(ctx.obj.get("verbose")) if ctx.obj else False
    return build_container(config_path=config_path, verbose=verbose)


def _augment_runtime_details(container: CliContainer, result: OperationResult) -> None:
    if not result.data:
        return

    doctor_report = container.settings.doctor_report()
    provider_capabilities = {
        "command_stt": container.command_stt.describe_capabilities(),
        "tts": container.speech_synthesizer.describe_capabilities(),
        "playback": container.playback_engine.describe_capabilities(),
    }
    requested_providers = {
        "command_stt": container.settings.command_stt_provider,
        "tts": container.settings.tts_provider,
        "playback": container.settings.playback_provider,
    }
    resolved_providers = {
        key: value.provider_name for key, value in provider_capabilities.items()
    }
    result.data["runtime_details"] = {
        "config_path": container.settings.config_path,
        "command_language": container.command_lexicon.language,
        "command_lexicon_path": container.command_lexicon.source_path,
        "runtime_record": container.runtime_supervisor.current_runtime(),
        "requested_providers": requested_providers,
        "resolved_providers": resolved_providers,
        "fallback_used": {
            "command_stt": requested_providers["command_stt"] != "fake"
            and resolved_providers["command_stt"].startswith("fake-"),
            "tts": requested_providers["tts"] != "fake"
            and resolved_providers["tts"].startswith("fake-"),
            "playback": requested_providers["playback"] != "fake"
            and resolved_providers["playback"].startswith("fake-"),
        },
        "audio_devices": {
            "default_input": doctor_report["provider_checks"]["vosk"]["selected_input_device"],
            "default_output": doctor_report["provider_checks"]["playback"]["default_output_device"],
            "bluetooth_output_active": doctor_report["provider_checks"]["playback"][
                "bluetooth_output_active"
            ],
        },
    }


@app.callback()
def main(
    ctx: typer.Context,
    config_path: Path | None = CONFIG_OPTION,
    verbose: bool = VERBOSE_OPTION,
) -> None:
    """Configure shared command context."""

    ctx.obj = {"config_path": config_path, "verbose": verbose}


@app.command()
def ingest(
    ctx: typer.Context,
    path: Path = DOCUMENT_PATH_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Ingest a text or markdown document into local storage."""

    container = _container_from_context(ctx)
    result = container.ingestion_service.ingest_text_file(path)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def play(
    ctx: typer.Context,
    target: str | None = PLAY_TARGET_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Ingest or select a document, then start the continuous read+listen runtime."""

    container = _container_from_context(ctx)
    loop = container.reading_runtime_service.create_loop()

    start_result = loop.start(target)
    if start_result.status is OperationStatus.ERROR:
        _augment_runtime_details(container, start_result)
        _emit_result(start_result, as_json=as_json)
        raise typer.Exit(code=1)

    prev_sigint = signal.getsignal(signal.SIGINT)
    prev_sigterm = signal.getsignal(signal.SIGTERM)

    def _handle_signal(signum: int, frame: object) -> None:
        loop.request_shutdown()

    signal.signal(signal.SIGINT, _handle_signal)
    signal.signal(signal.SIGTERM, _handle_signal)

    try:
        with loop:
            while loop.step() is StepStatus.CONTINUE:
                pass
    finally:
        signal.signal(signal.SIGINT, prev_sigint)
        signal.signal(signal.SIGTERM, prev_sigterm)

    result = loop.finalize()
    _augment_runtime_details(container, result)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def pause(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Pause the active reading session."""

    container = _container_from_context(ctx)
    result = container.reader_service.pause()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def resume(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Resume the active reading session."""

    container = _container_from_context(ctx)
    result = container.reader_service.resume()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("repeat")
def repeat_current(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Replay the current reading chunk."""

    container = _container_from_context(ctx)
    result = container.reader_service.repeat_current_chunk()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("restart-chapter")
def restart_chapter(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Move the session cursor to the start of the current chapter."""

    container = _container_from_context(ctx)
    result = container.reader_service.restart_chapter()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("next-chapter")
def next_chapter(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Move the session cursor to the next chapter."""

    container = _container_from_context(ctx)
    result = container.reader_service.next_chapter()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def stop(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Stop playback plus the active command-listening runtime."""

    container = _container_from_context(ctx)
    result = container.reading_runtime_service.stop()
    _augment_runtime_details(container, result)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("note-start")
def note_start(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Mark the active session as recording a note."""

    container = _container_from_context(ctx)
    result = container.note_service.start_note_capture()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("note-stop")
def note_stop(
    ctx: typer.Context,
    text: str | None = NOTE_TEXT_OPTION,
    as_json: bool = JSON_OPTION,
) -> None:
    """Persist the current note and anchor it to the session position."""

    container = _container_from_context(ctx)
    result = container.note_service.stop_note_capture(transcript=text)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("rewrite-current")
def rewrite_current(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Create a placeholder rewrite draft for the current section."""

    container = _container_from_context(ctx)
    result = container.rewrite_service.rewrite_current_section()
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("summarize-topic")
def summarize_topic(
    ctx: typer.Context,
    topic: str = TOPIC_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Summarize a topic inside the current local corpus using the fake provider."""

    container = _container_from_context(ctx)
    result = container.summary_service.summarize_topic(topic)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("search-document")
def search_document(
    ctx: typer.Context,
    query: str = QUERY_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Search document titles and stored outline text."""

    container = _container_from_context(ctx)
    result = container.search_service.search_documents(query)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command("search-notes")
def search_notes(
    ctx: typer.Context,
    query: str = QUERY_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Search locally stored notes."""

    container = _container_from_context(ctx)
    result = container.search_service.search_notes(query)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def status(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Report the active reading session state."""

    container = _container_from_context(ctx)
    result = container.session_query_service.current_status()
    _augment_runtime_details(container, result)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def doctor(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Report local configuration, database health, and provider readiness."""

    container = _container_from_context(ctx)
    report: dict[str, Any] = container.settings.doctor_report()
    report["database"] = container.database.health_report()
    provider_capabilities = {
        "command_stt": container.command_stt.describe_capabilities(),
        "dictation_stt": container.dictation_stt.describe_capabilities(),
        "tts": container.speech_synthesizer.describe_capabilities(),
        "playback": container.playback_engine.describe_capabilities(),
        "rewrite": container.rewrite_provider.describe_capabilities(),
        "summary": container.summary_provider.describe_capabilities(),
    }
    report["provider_capabilities"] = provider_capabilities
    report["resolved_providers"] = {
        key: value.provider_name for key, value in provider_capabilities.items()
    }
    report["command_lexicon"] = {
        "language": container.command_lexicon.language,
        "source_path": container.command_lexicon.source_path,
        "phrases": container.command_lexicon.grammar,
    }
    report["runtime"] = {
        "active_runtime": container.runtime_supervisor.current_runtime(),
        "uses_default_audio_devices": True,
    }
    result = OperationResult.ok("Marginalia CLI environment looks coherent.", data=report)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=0)


def run() -> None:
    """Console script entrypoint."""

    app()
