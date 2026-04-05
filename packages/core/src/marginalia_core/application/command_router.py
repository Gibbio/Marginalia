"""Load and resolve language-specific voice commands."""

from __future__ import annotations

import tomllib
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Any


class VoiceCommandIntent(str, Enum):
    """Canonical commands supported by the alpha voice loop."""

    PAUSE = "pause"
    RESUME = "resume"
    REPEAT = "repeat"
    NEXT_CHAPTER = "next-chapter"
    RESTART_CHAPTER = "restart-chapter"
    STATUS = "status"
    STOP = "stop"

    @property
    def config_key(self) -> str:
        """Return the TOML key used by language lexicon files."""

        return self.value.replace("-", "_")


@dataclass(frozen=True, slots=True)
class CommandLexicon:
    """Language-specific spoken phrases mapped to stable application intents."""

    language: str
    source_path: Path
    phrases_by_intent: dict[VoiceCommandIntent, tuple[str, ...]]
    phrase_to_intent: dict[str, VoiceCommandIntent]

    @property
    def grammar(self) -> tuple[str, ...]:
        """Return the normalized phrase set for grammar-limited recognizers."""

        phrases: list[str] = []
        for phrase in self.phrase_to_intent:
            phrases.append(phrase)
        return tuple(phrases)

    def resolve(self, raw_command: str) -> VoiceCommandIntent | None:
        """Map raw recognized text to a canonical application intent."""

        return self.phrase_to_intent.get(normalize_voice_command(raw_command))


def load_command_lexicon(source_path: Path) -> CommandLexicon:
    """Load a language-specific command lexicon from TOML."""

    if not source_path.exists():
        raise FileNotFoundError(f"Command lexicon '{source_path}' does not exist.")

    with source_path.open("rb") as handle:
        payload = tomllib.load(handle)

    language = str(payload.get("language") or source_path.stem).strip().lower()
    raw_intents = payload.get("intents")
    if not isinstance(raw_intents, dict):
        raise ValueError(f"Command lexicon '{source_path}' is missing an [intents] table.")

    phrases_by_intent: dict[VoiceCommandIntent, tuple[str, ...]] = {}
    phrase_to_intent: dict[str, VoiceCommandIntent] = {}
    for intent in VoiceCommandIntent:
        raw_phrases = raw_intents.get(intent.config_key)
        phrases = _coerce_phrases(raw_phrases, intent=intent, source_path=source_path)
        phrases_by_intent[intent] = phrases
        for phrase in phrases:
            normalized = normalize_voice_command(phrase)
            if normalized in phrase_to_intent and phrase_to_intent[normalized] is not intent:
                raise ValueError(
                    f"Phrase '{phrase}' is mapped to multiple intents in '{source_path}'."
                )
            phrase_to_intent[normalized] = intent

    return CommandLexicon(
        language=language,
        source_path=source_path.resolve(),
        phrases_by_intent=phrases_by_intent,
        phrase_to_intent=phrase_to_intent,
    )


def resolve_voice_command(
    raw_command: str,
    lexicon: CommandLexicon,
) -> VoiceCommandIntent | None:
    """Resolve a raw spoken phrase through the configured command lexicon."""

    return lexicon.resolve(raw_command)


def normalize_voice_command(raw_command: str) -> str:
    """Normalize spacing and casing for command matching."""

    return " ".join(raw_command.strip().lower().replace("_", " ").split())


def _coerce_phrases(
    raw_phrases: Any,
    *,
    intent: VoiceCommandIntent,
    source_path: Path,
) -> tuple[str, ...]:
    phrases: tuple[str, ...]
    if isinstance(raw_phrases, str):
        phrases = (raw_phrases,)
    elif isinstance(raw_phrases, list):
        phrases = tuple(str(item).strip() for item in raw_phrases if str(item).strip())
    elif isinstance(raw_phrases, tuple):
        phrases = tuple(str(item).strip() for item in raw_phrases if str(item).strip())
    else:
        phrases = ()

    if not phrases:
        raise ValueError(
            f"Command lexicon '{source_path}' must define at least one phrase for "
            f"intent '{intent.config_key}'."
        )
    return phrases
