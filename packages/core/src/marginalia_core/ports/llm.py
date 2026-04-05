"""LLM-facing ports for rewrite and summarization."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from marginalia_core.ports.capabilities import ProviderCapabilities


@dataclass(frozen=True, slots=True)
class RewriteInstruction:
    """Input passed to a rewrite provider."""

    document_title: str
    section_title: str
    source_anchor: str
    section_text: str
    note_texts: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class RewriteOutput:
    """Structured rewrite output."""

    provider_name: str
    rewritten_text: str
    strategy: str
    note_count: int


@dataclass(frozen=True, slots=True)
class SummaryInstruction:
    """Input passed to a summarization provider."""

    topic: str
    matched_document_ids: tuple[str, ...]
    context_excerpt: str = ""


@dataclass(frozen=True, slots=True)
class SummaryOutput:
    """Structured summary output."""

    provider_name: str
    summary_text: str
    highlights: tuple[str, ...] = ()
    confidence: float = 1.0


class RewriteGenerator(Protocol):
    """Generate a section rewrite from source text and notes."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe rewrite provider behavior and constraints."""
        ...

    def rewrite_section(self, instruction: RewriteInstruction) -> RewriteOutput:
        """Return rewritten text for a section."""
        ...


class TopicSummarizer(Protocol):
    """Summarize a topic across the local corpus."""

    def describe_capabilities(self) -> ProviderCapabilities:
        """Describe summarizer behavior and constraints."""
        ...

    def summarize_topic(self, instruction: SummaryInstruction) -> SummaryOutput:
        """Return a topic summary."""
        ...
