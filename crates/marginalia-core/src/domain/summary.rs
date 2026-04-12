use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryRequest {
    pub topic: String,
    pub document_id: Option<String>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SummaryResult {
    pub topic: String,
    pub summary_text: String,
    pub matched_document_ids: Vec<String>,
    pub highlights: Vec<String>,
    pub provider_name: String,
    pub generated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::{SummaryRequest, SummaryResult};
    use chrono::Utc;

    #[test]
    fn summary_models_hold_topic_and_matches() {
        let request = SummaryRequest {
            topic: "Roman history".to_string(),
            document_id: Some("doc-1".to_string()),
            requested_at: Utc::now(),
        };
        let result = SummaryResult {
            topic: request.topic.clone(),
            summary_text: "Summary".to_string(),
            matched_document_ids: vec!["doc-1".to_string()],
            highlights: vec!["Caesar".to_string()],
            provider_name: "fake".to_string(),
            generated_at: Utc::now(),
        };

        assert_eq!(result.topic, request.topic);
        assert_eq!(result.matched_document_ids, vec!["doc-1".to_string()]);
    }
}
