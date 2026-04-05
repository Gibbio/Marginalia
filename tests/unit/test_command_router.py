"""Voice command routing tests."""

from __future__ import annotations

from marginalia_core.application.command_router import VoiceCommandIntent, resolve_voice_command


def test_resolve_voice_command_maps_italian_vocabulary() -> None:
    assert resolve_voice_command("pausa") is VoiceCommandIntent.PAUSE
    assert resolve_voice_command("continua") is VoiceCommandIntent.RESUME
    assert resolve_voice_command("ripeti") is VoiceCommandIntent.REPEAT
    assert resolve_voice_command("capitolo successivo") is VoiceCommandIntent.NEXT_CHAPTER
    assert resolve_voice_command("ricomincia capitolo") is VoiceCommandIntent.RESTART_CHAPTER
    assert resolve_voice_command("stato") is VoiceCommandIntent.STATUS


def test_resolve_voice_command_normalizes_spaces_and_case() -> None:
    assert resolve_voice_command("  Capitolo   Successivo ") is VoiceCommandIntent.NEXT_CHAPTER
    assert resolve_voice_command("STOP") is VoiceCommandIntent.STOP
    assert resolve_voice_command("unknown") is None
