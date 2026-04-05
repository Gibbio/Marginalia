"""Typer application for Marginalia."""

from __future__ import annotations

import json
from dataclasses import asdict, is_dataclass
from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any

import typer

from marginalia_cli.bootstrap import CliContainer, build_container
from marginalia_core.application.result import OperationResult, OperationStatus

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
OPTIONAL_DOCUMENT_ID_ARGUMENT = typer.Argument(
    None,
    help="Document identifier. Uses the active or latest document if omitted.",
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
    document_id: str | None = OPTIONAL_DOCUMENT_ID_ARGUMENT,
    as_json: bool = JSON_OPTION,
) -> None:
    """Start or resume reading from a stored document."""

    container = _container_from_context(ctx)
    result = container.reader_service.play(document_id)
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
    """Return the current chunk anchor and text."""

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
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=_exit_code(result))


@app.command()
def doctor(
    ctx: typer.Context,
    as_json: bool = JSON_OPTION,
) -> None:
    """Report local configuration and placeholder provider wiring."""

    container = _container_from_context(ctx)
    report: dict[str, Any] = container.settings.doctor_report()
    report["database"] = container.database.health_report()
    result = OperationResult.ok("Marginalia CLI environment looks coherent.", data=report)
    _emit_result(result, as_json=as_json)
    raise typer.Exit(code=0)


def run() -> None:
    """Console script entrypoint."""

    app()
