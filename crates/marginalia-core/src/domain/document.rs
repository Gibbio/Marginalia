use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentChunk {
    pub index: usize,
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
}

impl DocumentChunk {
    pub fn anchor(&self) -> String {
        format!("chunk:{}", self.index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSection {
    pub index: usize,
    pub title: String,
    pub chunks: Vec<DocumentChunk>,
    pub source_anchor: Option<String>,
}

impl DocumentSection {
    pub fn text(&self) -> String {
        self.chunks
            .iter()
            .map(|chunk| chunk.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn get_chunk(&self, chunk_index: usize) -> Option<&DocumentChunk> {
        self.chunks.get(chunk_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub document_id: String,
    pub title: String,
    pub source_path: PathBuf,
    pub sections: Vec<DocumentSection>,
    pub imported_at: DateTime<Utc>,
}

impl Document {
    pub fn chapter_count(&self) -> usize {
        self.sections.len()
    }

    pub fn total_chunk_count(&self) -> usize {
        self.sections.iter().map(DocumentSection::chunk_count).sum()
    }

    pub fn get_section(&self, section_index: usize) -> Option<&DocumentSection> {
        self.sections.get(section_index)
    }

    pub fn get_chunk(&self, section_index: usize, chunk_index: usize) -> Option<&DocumentChunk> {
        self.get_section(section_index)
            .and_then(|section| section.get_chunk(chunk_index))
    }
}

#[cfg(test)]
mod tests {
    use super::{Document, DocumentChunk, DocumentSection};
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn section_text_joins_chunks_with_blank_lines() {
        let section = DocumentSection {
            index: 0,
            title: "Section".to_string(),
            chunks: vec![
                DocumentChunk {
                    index: 0,
                    text: "Alpha".to_string(),
                    char_start: 0,
                    char_end: 5,
                },
                DocumentChunk {
                    index: 1,
                    text: "Beta".to_string(),
                    char_start: 6,
                    char_end: 10,
                },
            ],
            source_anchor: Some("section:0".to_string()),
        };

        assert_eq!(section.text(), "Alpha\n\nBeta");
        assert_eq!(section.chunk_count(), 2);
        assert_eq!(section.get_chunk(1).map(|chunk| chunk.anchor()), Some("chunk:1".to_string()));
    }

    #[test]
    fn document_counts_sections_and_chunks() {
        let document = Document {
            document_id: "doc-1".to_string(),
            title: "Doc".to_string(),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![
                DocumentSection {
                    index: 0,
                    title: "One".to_string(),
                    chunks: vec![DocumentChunk {
                        index: 0,
                        text: "Alpha".to_string(),
                        char_start: 0,
                        char_end: 5,
                    }],
                    source_anchor: Some("section:0".to_string()),
                },
                DocumentSection {
                    index: 1,
                    title: "Two".to_string(),
                    chunks: vec![
                        DocumentChunk {
                            index: 0,
                            text: "Beta".to_string(),
                            char_start: 0,
                            char_end: 4,
                        },
                        DocumentChunk {
                            index: 1,
                            text: "Gamma".to_string(),
                            char_start: 5,
                            char_end: 10,
                        },
                    ],
                    source_anchor: Some("section:1".to_string()),
                },
            ],
            imported_at: Utc::now(),
        };

        assert_eq!(document.chapter_count(), 2);
        assert_eq!(document.total_chunk_count(), 3);
        assert_eq!(
            document
                .get_chunk(1, 1)
                .map(|chunk| chunk.text.as_str()),
            Some("Gamma")
        );
    }
}
