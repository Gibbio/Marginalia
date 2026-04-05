"""Map recognized voice commands to application intents."""

from __future__ import annotations

from enum import Enum


class VoiceCommandIntent(str, Enum):
    """Canonical commands supported by the alpha voice loop."""

    PAUSE = "pause"
    RESUME = "resume"
    REPEAT = "repeat"
    NEXT_CHAPTER = "next-chapter"
    RESTART_CHAPTER = "restart-chapter"
    STATUS = "status"
    STOP = "stop"


_VOICE_COMMAND_ALIASES: dict[str, VoiceCommandIntent] = {
    "pausa": VoiceCommandIntent.PAUSE,
    "pause": VoiceCommandIntent.PAUSE,
    "continua": VoiceCommandIntent.RESUME,
    "riprendi": VoiceCommandIntent.RESUME,
    "resume": VoiceCommandIntent.RESUME,
    "ripeti": VoiceCommandIntent.REPEAT,
    "repeat": VoiceCommandIntent.REPEAT,
    "capitolo successivo": VoiceCommandIntent.NEXT_CHAPTER,
    "next chapter": VoiceCommandIntent.NEXT_CHAPTER,
    "next-chapter": VoiceCommandIntent.NEXT_CHAPTER,
    "ricomincia capitolo": VoiceCommandIntent.RESTART_CHAPTER,
    "restart chapter": VoiceCommandIntent.RESTART_CHAPTER,
    "restart-chapter": VoiceCommandIntent.RESTART_CHAPTER,
    "stato": VoiceCommandIntent.STATUS,
    "status": VoiceCommandIntent.STATUS,
    "stop": VoiceCommandIntent.STOP,
}


def resolve_voice_command(raw_command: str) -> VoiceCommandIntent | None:
    """Map raw recognized text to a canonical application intent."""

    normalized = normalize_voice_command(raw_command)
    return _VOICE_COMMAND_ALIASES.get(normalized)


def normalize_voice_command(raw_command: str) -> str:
    """Normalize spacing and casing for command matching."""

    return " ".join(raw_command.strip().lower().replace("_", " ").split())
