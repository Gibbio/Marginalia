"""Topic summarization workflow."""

from __future__ import annotations

from marginalia_core.application.result import OperationResult
from marginalia_core.domain.summary import SummaryRequest, SummaryResult
from marginalia_core.events.models import DomainEvent, EventName
from marginalia_core.ports.events import EventPublisher
from marginalia_core.ports.llm import TopicSummarizer
from marginalia_core.ports.storage import DocumentRepository


class SummaryService:
    """Summarize topics using a replaceable summarizer port."""

    def __init__(
        self,
        *,
        document_repository: DocumentRepository,
        topic_summarizer: TopicSummarizer,
        event_publisher: EventPublisher,
    ) -> None:
        self._document_repository = document_repository
        self._topic_summarizer = topic_summarizer
        self._event_publisher = event_publisher

    def summarize_topic(self, topic: str) -> OperationResult:
        request = SummaryRequest(topic=topic)
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.SUMMARY_REQUESTED,
                payload={"topic": request.topic, "document_id": request.document_id},
            )
        )
        documents = self._document_repository.search_documents(request.topic)
        matched_document_ids = tuple(result.entity_id for result in documents)
        summary = SummaryResult(
            topic=request.topic,
            summary_text=self._topic_summarizer.summarize_topic(request.topic),
            matched_document_ids=matched_document_ids,
        )
        self._event_publisher.publish(
            DomainEvent(
                name=EventName.SUMMARY_COMPLETED,
                payload={
                    "topic": request.topic,
                    "matched_document_ids": matched_document_ids,
                },
            )
        )
        return OperationResult.ok(
            "Placeholder topic summary generated through the fake provider.",
            data={"summary": summary},
        )
