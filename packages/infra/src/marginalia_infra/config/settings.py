"""Application configuration loading."""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True, slots=True)
class AppSettings:
    """Local runtime settings for the bootstrap application."""

    app_name: str
    environment: str
    home_dir: Path
    data_dir: Path
    database_path: Path
    log_level: str
    command_stt_provider: str
    dictation_stt_provider: str
    tts_provider: str
    llm_provider: str
    config_path: Path | None = None

    @classmethod
    def load(cls, config_path: Path | None = None) -> "AppSettings":
        resolved_config = config_path or _config_path_from_env()
        config_data = _load_toml_file(resolved_config) if resolved_config else {}
        providers = config_data.get("providers", {})

        home_dir = _path_setting(
            env_key="MARGINALIA_HOME",
            config_data=config_data,
            config_key="home_dir",
            fallback=Path.home() / ".marginalia",
            base_dir=resolved_config.parent if resolved_config else None,
        )
        data_dir = _path_setting(
            env_key="MARGINALIA_DATA_DIR",
            config_data=config_data,
            config_key="data_dir",
            fallback=home_dir / "data",
            base_dir=resolved_config.parent if resolved_config else None,
        )
        database_path = _path_setting(
            env_key="MARGINALIA_DB_PATH",
            config_data=config_data,
            config_key="database_path",
            fallback=data_dir / "marginalia.sqlite3",
            base_dir=resolved_config.parent if resolved_config else None,
        )

        return cls(
            app_name="Marginalia",
            environment=os.getenv("MARGINALIA_ENV", str(config_data.get("environment", "local"))),
            home_dir=home_dir,
            data_dir=data_dir,
            database_path=database_path,
            log_level=os.getenv("MARGINALIA_LOG_LEVEL", str(config_data.get("log_level", "INFO"))),
            command_stt_provider=os.getenv(
                "MARGINALIA_COMMAND_STT_PROVIDER",
                str(providers.get("command_stt", "fake")),
            ),
            dictation_stt_provider=os.getenv(
                "MARGINALIA_DICTATION_STT_PROVIDER",
                str(providers.get("dictation_stt", "fake")),
            ),
            tts_provider=os.getenv("MARGINALIA_TTS_PROVIDER", str(providers.get("tts", "fake"))),
            llm_provider=os.getenv("MARGINALIA_LLM_PROVIDER", str(providers.get("llm", "fake"))),
            config_path=resolved_config,
        )

    def ensure_directories(self) -> None:
        database_parent = self.database_path.expanduser().resolve(strict=False).parent
        data_dir = self.data_dir.expanduser().resolve(strict=False)

        if database_parent == data_dir or database_parent.is_relative_to(data_dir):
            self.data_dir.mkdir(parents=True, exist_ok=True)

        self.database_path.parent.mkdir(parents=True, exist_ok=True)

    def doctor_report(self) -> dict[str, Any]:
        return {
            "app_name": self.app_name,
            "environment": self.environment,
            "paths": {
                "home_dir": self.home_dir,
                "data_dir": self.data_dir,
                "database_path": self.database_path,
                "config_path": self.config_path,
            },
            "providers": {
                "command_stt": self.command_stt_provider,
                "dictation_stt": self.dictation_stt_provider,
                "tts": self.tts_provider,
                "llm": self.llm_provider,
            },
        }


def _config_path_from_env() -> Path | None:
    value = os.getenv("MARGINALIA_CONFIG")
    return Path(value).expanduser() if value else None


def _load_toml_file(config_path: Path) -> dict[str, Any]:
    if not config_path.exists():
        return {}
    with config_path.open("rb") as handle:
        data = tomllib.load(handle)
    return data if isinstance(data, dict) else {}


def _path_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    fallback: Path,
    base_dir: Path | None,
) -> Path:
    if env_value := os.getenv(env_key):
        return Path(env_value).expanduser()
    if config_key not in config_data:
        return fallback.expanduser()
    value = Path(str(config_data[config_key])).expanduser()
    if not value.is_absolute() and base_dir is not None:
        return (base_dir / value).resolve()
    return value
