"""Fake LLM adapters."""

from __future__ import annotations

from collections.abc import Sequence


class FakeRewriteGenerator:
    """Return explicit placeholder rewrite text."""

    def rewrite_section(self, section_text: str, note_texts: Sequence[str]) -> str:
        note_summary = "; ".join(note_texts)
        return (
            "FAKE REWRITE\n\n"
            f"Original excerpt:\n{section_text[:400]}\n\n"
            f"Notes considered:\n{note_summary}\n"
        )


class FakeTopicSummarizer:
    """Return explicit placeholder summaries."""

    def summarize_topic(self, topic: str) -> str:
        return (
            f"Fake summary for topic '{topic}'. "
            "Replace the summarizer port with a real provider later."
        )
