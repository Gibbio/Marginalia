use crate::ports::capabilities::ProviderCapabilities;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteInstruction {
    pub document_title: String,
    pub section_title: String,
    pub source_anchor: String,
    pub section_text: String,
    pub note_texts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteOutput {
    pub provider_name: String,
    pub rewritten_text: String,
    pub strategy: String,
    pub note_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryInstruction {
    pub topic: String,
    pub matched_document_ids: Vec<String>,
    pub context_excerpt: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryOutput {
    pub provider_name: String,
    pub summary_text: String,
    pub highlights: Vec<String>,
    pub confidence: f64,
}

pub trait RewriteGenerator {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn rewrite_section(&self, instruction: RewriteInstruction) -> RewriteOutput;
}

pub trait TopicSummarizer {
    fn describe_capabilities(&self) -> ProviderCapabilities;
    fn summarize_topic(&self, instruction: SummaryInstruction) -> SummaryOutput;
}
