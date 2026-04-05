"""Fake LLM adapters."""

from __future__ import annotations

from marginalia_core.ports.capabilities import ProviderCapabilities
from marginalia_core.ports.llm import (
    RewriteInstruction,
    RewriteOutput,
    SummaryInstruction,
    SummaryOutput,
)

REWRITE_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-rewrite-llm",
    interface_kind="rewrite-provider",
    supported_languages=("en",),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=False,
    offline_capable=True,
)

SUMMARY_CAPABILITIES = ProviderCapabilities(
    provider_name="fake-summary-llm",
    interface_kind="summary-provider",
    supported_languages=("en",),
    supports_streaming=False,
    supports_partial_results=False,
    supports_timestamps=False,
    low_latency_suitable=False,
    offline_capable=True,
)


class FakeRewriteGenerator:
    """Return deterministic rewrite drafts."""

    def describe_capabilities(self) -> ProviderCapabilities:
        return REWRITE_CAPABILITIES

    def rewrite_section(self, instruction: RewriteInstruction) -> RewriteOutput:
        note_summary = "; ".join(instruction.note_texts)
        rewritten_text = (
            f"FAKE REWRITE for {instruction.section_title}\n\n"
            f"Anchor: {instruction.source_anchor}\n"
            f"Original excerpt:\n{instruction.section_text[:400]}\n\n"
            f"Notes considered:\n{note_summary}\n"
        )
        return RewriteOutput(
            provider_name=REWRITE_CAPABILITIES.provider_name,
            rewritten_text=rewritten_text,
            strategy="deterministic-annotated-rewrite",
            note_count=len(instruction.note_texts),
        )


class FakeTopicSummarizer:
    """Return explicit deterministic summaries."""

    def describe_capabilities(self) -> ProviderCapabilities:
        return SUMMARY_CAPABILITIES

    def summarize_topic(self, instruction: SummaryInstruction) -> SummaryOutput:
        summary_text = (
            f"Fake summary for topic '{instruction.topic}' across "
            f"{len(instruction.matched_document_ids)} local document(s)."
        )
        highlights = (
            f"Topic: {instruction.topic}",
            f"Matched documents: {len(instruction.matched_document_ids)}",
            "Provider remains a deterministic local fake adapter.",
        )
        return SummaryOutput(
            provider_name=SUMMARY_CAPABILITIES.provider_name,
            summary_text=summary_text,
            highlights=highlights,
            confidence=0.42,
        )
