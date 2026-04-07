"""Typer application for the headless Marginalia backend."""

from __future__ import annotations

import json
from dataclasses import asdict
from pathlib import Path

import typer

from marginalia_backend.bootstrap import build_backend_container
from marginalia_backend.gateway import LocalFrontendGateway
from marginalia_backend.serialization import to_transport_value
from marginalia_backend.stdio_server import StdioFrontendServer

app = typer.Typer(
    name="marginalia-backend",
    help="Headless local backend for Marginalia frontends.",
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
VERBOSE_OPTION = typer.Option(False, "--verbose", help="Enable verbose backend logging.")


def _gateway_from_options(
    *,
    config_path: Path | None,
    verbose: bool,
) -> LocalFrontendGateway:
    container = build_backend_container(config_path=config_path, verbose=verbose)
    return LocalFrontendGateway(container)


@app.command("describe-contract")
def describe_contract(
    config_path: Path | None = CONFIG_OPTION,
    verbose: bool = VERBOSE_OPTION,
) -> None:
    """Emit backend capabilities and supported contract surface as JSON."""

    gateway = _gateway_from_options(config_path=config_path, verbose=verbose)
    typer.echo(json.dumps(to_transport_value(asdict(gateway.capabilities())), indent=2))


@app.command("serve-stdio")
def serve_stdio(
    config_path: Path | None = CONFIG_OPTION,
    verbose: bool = VERBOSE_OPTION,
) -> None:
    """Serve the frontend contract over stdio using JSON Lines."""

    gateway = _gateway_from_options(config_path=config_path, verbose=verbose)
    try:
        StdioFrontendServer(gateway).serve_forever()
    finally:
        gateway.shutdown()


def run() -> None:
    """Console script entrypoint."""

    app()
