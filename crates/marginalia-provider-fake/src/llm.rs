use marginalia_core::ports::{
    ProviderCapabilities, ProviderExecutionMode, RewriteGenerator, RewriteInstruction,
    RewriteOutput, SummaryInstruction, SummaryOutput, TopicSummarizer,
};

#[derive(Debug, Clone, Default)]
pub struct FakeRewriteGenerator;

impl FakeRewriteGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl RewriteGenerator for FakeRewriteGenerator {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "fake-rewrite".to_string(),
            interface_kind: "rewrite".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: false,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn rewrite_section(&self, instruction: RewriteInstruction) -> RewriteOutput {
        let notes = if instruction.note_texts.is_empty() {
            "no notes".to_string()
        } else {
            instruction.note_texts.join(" | ")
        };

        RewriteOutput {
            provider_name: "fake-rewrite".to_string(),
            rewritten_text: format!(
                "[{} / {}] {} || notes: {}",
                instruction.document_title,
                instruction.section_title,
                instruction.section_text,
                notes
            ),
            strategy: "deterministic-template".to_string(),
            note_count: instruction.note_texts.len(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FakeTopicSummarizer;

impl FakeTopicSummarizer {
    pub fn new() -> Self {
        Self
    }
}

impl TopicSummarizer for FakeTopicSummarizer {
    fn describe_capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            provider_name: "fake-summary".to_string(),
            interface_kind: "summary".to_string(),
            supported_languages: vec!["it".to_string(), "en".to_string()],
            supports_streaming: false,
            supports_partial_results: false,
            supports_timestamps: false,
            low_latency_suitable: false,
            offline_capable: true,
            execution_mode: ProviderExecutionMode::Local,
        }
    }

    fn summarize_topic(&self, instruction: SummaryInstruction) -> SummaryOutput {
        SummaryOutput {
            provider_name: "fake-summary".to_string(),
            summary_text: format!(
                "Topic: {} | Documents: {} | Context: {}",
                instruction.topic,
                instruction.matched_document_ids.join(","),
                instruction.context_excerpt
            ),
            highlights: vec![instruction.topic],
            confidence: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FakeRewriteGenerator, FakeTopicSummarizer};
    use marginalia_core::ports::{
        RewriteGenerator, RewriteInstruction, SummaryInstruction, TopicSummarizer,
    };

    #[test]
    fn fake_rewrite_generator_returns_deterministic_output() {
        let generator = FakeRewriteGenerator::new();
        let output = generator.rewrite_section(RewriteInstruction {
            document_title: "Doc".to_string(),
            section_title: "Intro".to_string(),
            source_anchor: "section:0".to_string(),
            section_text: "Alpha beta".to_string(),
            note_texts: vec!["note".to_string()],
        });

        assert_eq!(output.provider_name, "fake-rewrite");
        assert_eq!(output.note_count, 1);
        assert!(output.rewritten_text.contains("Alpha beta"));
    }

    #[test]
    fn fake_topic_summarizer_returns_deterministic_output() {
        let summarizer = FakeTopicSummarizer::new();
        let output = summarizer.summarize_topic(SummaryInstruction {
            topic: "Roman history".to_string(),
            matched_document_ids: vec!["doc-1".to_string()],
            context_excerpt: "Caesar crossed the Rubicon.".to_string(),
        });

        assert_eq!(output.provider_name, "fake-summary");
        assert_eq!(output.highlights, vec!["Roman history".to_string()]);
    }
}
