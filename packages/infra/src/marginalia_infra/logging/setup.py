"""Logging configuration."""

from __future__ import annotations

import logging
from pathlib import Path


def configure_logging(
    level: str = "INFO",
    *,
    log_file: Path | None = None,
) -> None:
    """Initialize application logging with structured local format."""

    root_logger = logging.getLogger()
    if root_logger.handlers:
        return

    resolved_level = getattr(logging, level.upper(), logging.INFO)
    formatter = logging.Formatter(
        "%(asctime)s %(levelname)s %(name)s %(message)s",
        datefmt="%Y-%m-%dT%H:%M:%S",
    )

    console_handler = logging.StreamHandler()
    console_handler.setLevel(resolved_level)
    console_handler.setFormatter(formatter)
    root_logger.addHandler(console_handler)

    if log_file is not None:
        log_file.parent.mkdir(parents=True, exist_ok=True)
        file_handler = logging.FileHandler(log_file, encoding="utf-8")
        file_handler.setLevel(logging.DEBUG)
        file_handler.setFormatter(formatter)
        root_logger.addHandler(file_handler)

    root_logger.setLevel(min(resolved_level, logging.DEBUG) if log_file else resolved_level)

    logging.getLogger("marginalia_core").setLevel(resolved_level)
    logging.getLogger("marginalia_adapters").setLevel(resolved_level)
    logging.getLogger("marginalia_infra").setLevel(resolved_level)
    logging.getLogger("marginalia_cli").setLevel(resolved_level)
