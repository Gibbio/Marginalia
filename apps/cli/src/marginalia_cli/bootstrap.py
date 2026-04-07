"""Compatibility shim for CLI imports during backend extraction."""

from marginalia_backend.bootstrap import (
    BackendContainer as CliContainer,
    build_backend_container as build_container,
)
