"""Voice command lexicon tests."""

from __future__ import annotations

from pathlib import Path

from marginalia_core.application.command_router import (
    VoiceCommandIntent,
    load_command_lexicon,
    resolve_voice_command,
)


def test_load_command_lexicon_maps_italian_vocabulary() -> None:
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )

    assert lexicon.language == "it"
    assert resolve_voice_command("pausa", lexicon) is VoiceCommandIntent.PAUSE
    assert resolve_voice_command("continua", lexicon) is VoiceCommandIntent.RESUME
    assert resolve_voice_command("ripeti", lexicon) is VoiceCommandIntent.REPEAT
    assert resolve_voice_command("capitolo successivo", lexicon) is VoiceCommandIntent.NEXT_CHAPTER
    assert (
        resolve_voice_command("ricomincia capitolo", lexicon)
        is VoiceCommandIntent.RESTART_CHAPTER
    )
    assert resolve_voice_command("stato", lexicon) is VoiceCommandIntent.STATUS


def test_command_lexicon_normalizes_spaces_and_case() -> None:
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/it.toml")
    )

    assert (
        resolve_voice_command("  Capitolo   Successivo ", lexicon)
        is VoiceCommandIntent.NEXT_CHAPTER
    )
    assert resolve_voice_command("STOP", lexicon) is VoiceCommandIntent.STOP
    assert resolve_voice_command("unknown", lexicon) is None


def test_load_command_lexicon_supports_english_variants() -> None:
    lexicon = load_command_lexicon(
        Path("packages/infra/src/marginalia_infra/config/commands/en.toml")
    )

    assert lexicon.language == "en"
    assert resolve_voice_command("pause", lexicon) is VoiceCommandIntent.PAUSE
    assert resolve_voice_command("continue", lexicon) is VoiceCommandIntent.RESUME
    assert resolve_voice_command("next chapter", lexicon) is VoiceCommandIntent.NEXT_CHAPTER
