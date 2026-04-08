#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub document_id: Option<String>,
    pub limit: usize,
}

impl SearchQuery {
    pub fn normalized_text(&self) -> String {
        self.text.trim().to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub entity_kind: String,
    pub entity_id: String,
    pub score: f64,
    pub excerpt: String,
    pub anchor: String,
}

#[cfg(test)]
mod tests {
    use super::SearchQuery;

    #[test]
    fn normalized_text_trims_query() {
        let query = SearchQuery {
            text: "  some topic  ".to_string(),
            document_id: None,
            limit: 10,
        };

        assert_eq!(query.normalized_text(), "some topic");
    }
}
