"""Configuration loading tests."""

from __future__ import annotations

from pathlib import Path

from pytest import MonkeyPatch

from marginalia_infra.config.settings import AppSettings


def test_settings_respect_environment_paths(tmp_path: Path, monkeypatch: MonkeyPatch) -> None:
    home_path = tmp_path / "home"
    database_path = tmp_path / "custom.sqlite3"
    monkeypatch.setenv("MARGINALIA_HOME", str(home_path))
    monkeypatch.setenv("MARGINALIA_DB_PATH", str(database_path))

    settings = AppSettings.load()

    assert settings.home_dir == home_path
    assert settings.database_path == database_path
    assert settings.data_dir == home_path / "data"
