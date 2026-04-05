"""Application configuration loading."""

from __future__ import annotations

import importlib.util
import os
import shutil
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

DEFAULT_ITALIAN_COMMANDS = (
    "pausa",
    "continua",
    "ripeti",
    "capitolo successivo",
    "ricomincia capitolo",
    "stato",
    "stop",
)


@dataclass(frozen=True, slots=True)
class AppSettings:
    """Local runtime settings for the bootstrap application."""

    app_name: str
    environment: str
    home_dir: Path
    data_dir: Path
    database_path: Path
    audio_cache_dir: Path
    log_level: str
    command_stt_provider: str
    dictation_stt_provider: str
    tts_provider: str
    playback_provider: str
    llm_provider: str
    allow_provider_fallback: bool
    fake_command_script: tuple[str, ...]
    fake_dictation_text: str
    default_voice: str
    piper_executable: str
    piper_model_path: Path | None
    piper_speaker_id: int | None
    piper_length_scale: float
    piper_noise_scale: float
    vosk_model_path: Path | None
    vosk_sample_rate: int
    vosk_command_grammar: tuple[str, ...]
    vosk_listen_timeout_seconds: float
    playback_command: str
    config_path: Path | None = None

    @classmethod
    def load(cls, config_path: Path | None = None) -> AppSettings:
        resolved_config = config_path or _config_path_from_env()
        config_data = _load_toml_file(resolved_config) if resolved_config else {}
        providers = _as_dict(config_data.get("providers"))
        fake_providers = _as_dict(config_data.get("fake_providers"))
        piper = _as_dict(config_data.get("piper"))
        vosk = _as_dict(config_data.get("vosk"))
        playback = _as_dict(config_data.get("playback"))

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
        if (
            os.getenv("MARGINALIA_DATA_DIR") is None
            and os.getenv("MARGINALIA_HOME") is None
            and "data_dir" not in config_data
            and os.getenv("MARGINALIA_DB_PATH") is not None
        ):
            data_dir = database_path.parent
        audio_cache_dir = _path_setting(
            env_key="MARGINALIA_AUDIO_CACHE_DIR",
            config_data=config_data,
            config_key="audio_cache_dir",
            fallback=data_dir / "audio-cache",
            base_dir=resolved_config.parent if resolved_config else None,
        )

        requested_tts_provider = os.getenv(
            "MARGINALIA_TTS_PROVIDER", str(providers.get("tts", "fake"))
        )
        return cls(
            app_name="Marginalia",
            environment=os.getenv("MARGINALIA_ENV", str(config_data.get("environment", "local"))),
            home_dir=home_dir,
            data_dir=data_dir,
            database_path=database_path,
            audio_cache_dir=audio_cache_dir,
            log_level=os.getenv("MARGINALIA_LOG_LEVEL", str(config_data.get("log_level", "INFO"))),
            command_stt_provider=os.getenv(
                "MARGINALIA_COMMAND_STT_PROVIDER",
                str(providers.get("command_stt", "fake")),
            ),
            dictation_stt_provider=os.getenv(
                "MARGINALIA_DICTATION_STT_PROVIDER",
                str(providers.get("dictation_stt", "fake")),
            ),
            tts_provider=requested_tts_provider,
            playback_provider=os.getenv(
                "MARGINALIA_PLAYBACK_PROVIDER",
                str(
                    providers.get(
                        "playback", "subprocess" if requested_tts_provider == "piper" else "fake"
                    )
                ),
            ),
            llm_provider=os.getenv("MARGINALIA_LLM_PROVIDER", str(providers.get("llm", "fake"))),
            allow_provider_fallback=_bool_setting(
                env_key="MARGINALIA_ALLOW_PROVIDER_FALLBACK",
                config_data=providers,
                config_key="allow_fallback",
                fallback=True,
            ),
            fake_command_script=_tuple_setting(
                env_key="MARGINALIA_FAKE_COMMANDS",
                config_data=fake_providers,
                config_key="commands",
            ),
            fake_dictation_text=os.getenv(
                "MARGINALIA_FAKE_DICTATION_TEXT",
                str(fake_providers.get("dictation_text", "Placeholder dictated note.")),
            ),
            default_voice=os.getenv(
                "MARGINALIA_DEFAULT_VOICE",
                str(config_data.get("default_voice", "marginalia-default")),
            ),
            piper_executable=os.getenv(
                "MARGINALIA_PIPER_EXECUTABLE",
                str(piper.get("executable", "piper")),
            ),
            piper_model_path=_optional_path_setting(
                env_key="MARGINALIA_PIPER_MODEL_PATH",
                config_data=piper,
                config_key="model_path",
                base_dir=resolved_config.parent if resolved_config else None,
            ),
            piper_speaker_id=_optional_int_setting(
                env_key="MARGINALIA_PIPER_SPEAKER_ID",
                config_data=piper,
                config_key="speaker_id",
            ),
            piper_length_scale=_float_setting(
                env_key="MARGINALIA_PIPER_LENGTH_SCALE",
                config_data=piper,
                config_key="length_scale",
                fallback=1.0,
            ),
            piper_noise_scale=_float_setting(
                env_key="MARGINALIA_PIPER_NOISE_SCALE",
                config_data=piper,
                config_key="noise_scale",
                fallback=0.667,
            ),
            vosk_model_path=_optional_path_setting(
                env_key="MARGINALIA_VOSK_MODEL_PATH",
                config_data=vosk,
                config_key="model_path",
                base_dir=resolved_config.parent if resolved_config else None,
            ),
            vosk_sample_rate=_int_setting(
                env_key="MARGINALIA_VOSK_SAMPLE_RATE",
                config_data=vosk,
                config_key="sample_rate",
                fallback=16_000,
            ),
            vosk_command_grammar=_tuple_setting(
                env_key="MARGINALIA_VOSK_COMMANDS",
                config_data=vosk,
                config_key="commands",
                fallback=DEFAULT_ITALIAN_COMMANDS,
            ),
            vosk_listen_timeout_seconds=_float_setting(
                env_key="MARGINALIA_VOSK_TIMEOUT_SECONDS",
                config_data=vosk,
                config_key="timeout_seconds",
                fallback=4.0,
            ),
            playback_command=os.getenv(
                "MARGINALIA_PLAYBACK_COMMAND",
                str(playback.get("command", "afplay")),
            ),
            config_path=resolved_config,
        )

    def ensure_directories(self) -> None:
        database_parent = self.database_path.expanduser().resolve(strict=False).parent
        data_dir = self.data_dir.expanduser().resolve(strict=False)
        audio_cache_dir = self.audio_cache_dir.expanduser().resolve(strict=False)

        if database_parent == data_dir or database_parent.is_relative_to(data_dir):
            self.data_dir.mkdir(parents=True, exist_ok=True)

        self.database_path.parent.mkdir(parents=True, exist_ok=True)
        audio_cache_dir.mkdir(parents=True, exist_ok=True)

    def doctor_report(self) -> dict[str, Any]:
        piper_available = shutil.which(self.piper_executable) is not None
        playback_command_available = shutil.which(self.playback_command) is not None
        vosk_package_available = importlib.util.find_spec("vosk") is not None
        sounddevice_package_available = importlib.util.find_spec("sounddevice") is not None
        return {
            "app_name": self.app_name,
            "environment": self.environment,
            "paths": {
                "home_dir": self.home_dir,
                "data_dir": self.data_dir,
                "database_path": self.database_path,
                "audio_cache_dir": self.audio_cache_dir,
                "config_path": self.config_path,
                "database_parent_exists": self.database_path.parent.exists(),
            },
            "providers": {
                "command_stt": self.command_stt_provider,
                "dictation_stt": self.dictation_stt_provider,
                "tts": self.tts_provider,
                "playback": self.playback_provider,
                "llm": self.llm_provider,
                "allow_fallback": self.allow_provider_fallback,
            },
            "fake_providers": {
                "command_script": self.fake_command_script,
                "dictation_text": self.fake_dictation_text,
            },
            "defaults": {
                "voice": self.default_voice,
                "vosk_command_grammar": self.vosk_command_grammar,
            },
            "provider_checks": {
                "piper": {
                    "executable": self.piper_executable,
                    "executable_available": piper_available,
                    "model_path": self.piper_model_path,
                    "model_exists": bool(self.piper_model_path and self.piper_model_path.exists()),
                    "speaker_id": self.piper_speaker_id,
                    "ready": piper_available
                    and bool(self.piper_model_path and self.piper_model_path.exists()),
                },
                "vosk": {
                    "model_path": self.vosk_model_path,
                    "model_exists": bool(self.vosk_model_path and self.vosk_model_path.exists()),
                    "python_package_available": vosk_package_available,
                    "sounddevice_package_available": sounddevice_package_available,
                    "sample_rate": self.vosk_sample_rate,
                    "timeout_seconds": self.vosk_listen_timeout_seconds,
                    "ready": bool(self.vosk_model_path and self.vosk_model_path.exists())
                    and vosk_package_available
                    and sounddevice_package_available,
                },
                "playback": {
                    "command": self.playback_command,
                    "command_available": playback_command_available,
                    "ready": playback_command_available,
                },
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


def _optional_path_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    base_dir: Path | None,
) -> Path | None:
    if env_value := os.getenv(env_key):
        return Path(env_value).expanduser()
    raw_value = config_data.get(config_key)
    if raw_value in {None, ""}:
        return None
    value = Path(str(raw_value)).expanduser()
    if not value.is_absolute() and base_dir is not None:
        return (base_dir / value).resolve()
    return value


def _tuple_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    fallback: tuple[str, ...] = (),
) -> tuple[str, ...]:
    if env_value := os.getenv(env_key):
        return tuple(item.strip() for item in env_value.split(",") if item.strip())

    config_value = config_data.get(config_key, fallback)
    if isinstance(config_value, str):
        return tuple(item.strip() for item in config_value.split(",") if item.strip())
    if isinstance(config_value, list):
        return tuple(str(item) for item in config_value if str(item).strip())
    if isinstance(config_value, tuple):
        return tuple(str(item) for item in config_value if str(item).strip())
    return fallback


def _int_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    fallback: int,
) -> int:
    raw_value = os.getenv(env_key)
    if raw_value is None:
        raw_value = config_data.get(config_key, fallback)
    return int(raw_value)


def _optional_int_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
) -> int | None:
    raw_value = os.getenv(env_key)
    if raw_value is None:
        raw_value = config_data.get(config_key)
    if raw_value in {None, ""}:
        return None
    return int(str(raw_value))


def _float_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    fallback: float,
) -> float:
    raw_value = os.getenv(env_key)
    if raw_value is None:
        raw_value = config_data.get(config_key, fallback)
    return float(raw_value)


def _bool_setting(
    *,
    env_key: str,
    config_data: dict[str, Any],
    config_key: str,
    fallback: bool,
) -> bool:
    raw_value = os.getenv(env_key)
    if raw_value is None:
        raw_value = config_data.get(config_key, fallback)
    if isinstance(raw_value, bool):
        return raw_value
    normalized = str(raw_value).strip().lower()
    return normalized in {"1", "true", "yes", "on"}


def _as_dict(value: object) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}
