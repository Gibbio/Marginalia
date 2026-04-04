"""LLM-facing ports for rewrite and summarization."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Protocol


class RewriteGenerator(Protocol):
    """Generate a section rewrite from source text and notes."""

    def rewrite_section(self, section_text: str, note_texts: Sequence[str]) -> str:
        """Return rewritten text for a section."""
        ...


class TopicSummarizer(Protocol):
    """Summarize a topic across the local corpus."""

    def summarize_topic(self, topic: str) -> str:
        """Return a topic summary."""
        ...
