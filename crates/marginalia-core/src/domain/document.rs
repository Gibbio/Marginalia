use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Default target size in characters for text chunks (~300).
pub const DEFAULT_CHUNK_TARGET_CHARS: usize = 300;

/// A section of raw imported text before chunking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSection {
    /// Section heading text.
    pub title: String,
    /// Raw paragraphs within this section.
    pub paragraphs: Vec<String>,
    /// Optional anchor linking back to the source document location.
    pub source_anchor: Option<String>,
}

/// A raw document as returned by the importer, before chunking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedDocument {
    /// Document title extracted from the source, if available.
    pub title: Option<String>,
    /// Filesystem path of the source file.
    pub source_path: PathBuf,
    /// Sections of raw imported text.
    pub sections: Vec<ImportedSection>,
}

/// A chunk of text within a document section, sized for TTS synthesis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentChunk {
    /// Zero-based index of this chunk within its section.
    pub index: usize,
    /// The chunk's text content.
    pub text: String,
    /// Character offset where this chunk starts in the section text.
    pub char_start: usize,
    /// Character offset where this chunk ends in the section text.
    pub char_end: usize,
}

impl DocumentChunk {
    pub fn anchor(&self) -> String {
        format!("chunk:{}", self.index)
    }
}

/// A section (chapter) of a document, containing one or more chunks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSection {
    /// Zero-based index of this section within the document.
    pub index: usize,
    /// Section heading text.
    pub title: String,
    /// Ordered chunks of text within this section.
    pub chunks: Vec<DocumentChunk>,
    /// Optional anchor linking back to the source document location.
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

/// A fully imported and chunked document, ready for reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Unique identifier (truncated SHA-256 hash).
    pub document_id: String,
    /// Document title (from source or derived from filename).
    pub title: String,
    /// Filesystem path of the original source file.
    pub source_path: PathBuf,
    /// Ordered sections (chapters) of the document.
    pub sections: Vec<DocumentSection>,
    /// Timestamp when the document was imported.
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

impl ImportedDocument {
    pub fn canonical_text(&self) -> String {
        self.sections
            .iter()
            .map(|section| {
                let body = section
                    .paragraphs
                    .iter()
                    .map(|paragraph| paragraph.trim())
                    .filter(|paragraph| !paragraph.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n\n");

                if section.title.trim().is_empty() {
                    body
                } else if body.is_empty() {
                    section.title.trim().to_string()
                } else {
                    format!("{}\n\n{}", section.title.trim(), body)
                }
            })
            .filter(|section_text| !section_text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Build a `Document` from an `ImportedDocument` by chunking each section.
pub fn build_document_from_import(
    imported: ImportedDocument,
    chunk_target_chars: usize,
) -> Document {
    let title = imported
        .title
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| title_from_path(&imported.source_path));

    let chunk_target_chars = chunk_target_chars.max(1);
    let mut sections = imported
        .sections
        .into_iter()
        .enumerate()
        .map(|(index, section)| {
            let section_title = if section.title.trim().is_empty() {
                format!("Section {}", index + 1)
            } else {
                section.title.trim().to_string()
            };

            let section_text = section
                .paragraphs
                .iter()
                .map(|paragraph| paragraph.trim())
                .filter(|paragraph| !paragraph.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");

            DocumentSection {
                index,
                title: section_title,
                chunks: chunk_section_text(&section_text, chunk_target_chars),
                source_anchor: section
                    .source_anchor
                    .or_else(|| Some(format!("section:{}", index))),
            }
        })
        .collect::<Vec<_>>();

    if sections.is_empty() {
        sections.push(DocumentSection {
            index: 0,
            title: title.clone(),
            chunks: chunk_section_text("", chunk_target_chars),
            source_anchor: Some("section:0".to_string()),
        });
    }

    let document_hash_input = format!(
        "{}::{}",
        imported.source_path.to_string_lossy(),
        ImportedDocument {
            title: Some(title.clone()),
            source_path: imported.source_path.clone(),
            sections: sections
                .iter()
                .map(|section| ImportedSection {
                    title: section.title.clone(),
                    paragraphs: vec![section.text()],
                    source_anchor: section.source_anchor.clone(),
                })
                .collect(),
        }
        .canonical_text()
    );
    let document_id = format!("{:x}", Sha256::digest(document_hash_input.as_bytes()));

    Document {
        document_id: document_id[..12].to_string(),
        title,
        source_path: imported.source_path,
        sections,
        imported_at: Utc::now(),
    }
}

fn title_from_path(source_path: &Path) -> String {
    let stem = source_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled Document");

    let mut titled_words = Vec::new();
    for word in stem.split(['-', '_', ' ']) {
        if word.is_empty() {
            continue;
        }

        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            let rest = chars.collect::<String>().to_lowercase();
            titled_words.push(format!("{}{}", first.to_uppercase(), rest));
        }
    }

    if titled_words.is_empty() {
        "Untitled Document".to_string()
    } else {
        titled_words.join(" ")
    }
}

fn chunk_section_text(section_text: &str, chunk_target_chars: usize) -> Vec<DocumentChunk> {
    let fragments = locate_paragraphs(section_text);
    if fragments.is_empty() {
        return vec![DocumentChunk {
            index: 0,
            text: String::new(),
            char_start: 0,
            char_end: 0,
        }];
    }

    // Split every paragraph at sentence boundaries so merge_fragments works
    // with sentence-level units and can honour sentence-boundary flushing.
    let mut expanded = Vec::new();
    for (text, start, _end) in fragments {
        expanded.extend(split_at_sentences(&text, start));
    }

    merge_fragments(expanded, chunk_target_chars)
        .into_iter()
        .enumerate()
        .map(|(index, (text, char_start, char_end))| DocumentChunk {
            index,
            text,
            char_start,
            char_end,
        })
        .collect()
}

fn locate_paragraphs(section_text: &str) -> Vec<(String, usize, usize)> {
    let mut result = Vec::new();
    let mut search_start = 0usize;

    for raw in section_text.split("\n\n") {
        let stripped = raw.trim();
        if stripped.is_empty() {
            search_start = search_start.saturating_add(raw.len() + 2);
            continue;
        }

        let relative = section_text[search_start..].find(stripped).unwrap_or(0);
        let char_start = search_start + relative;
        let char_end = char_start + stripped.len();
        result.push((stripped.to_string(), char_start, char_end));
        search_start = char_end;
    }

    result
}

fn split_at_sentences(text: &str, base_offset: usize) -> Vec<(String, usize, usize)> {
    let mut fragments = Vec::new();
    let mut sentence_start = 0usize;

    for (index, current) in text.char_indices() {
        if !matches!(current, '.' | '!' | '?' | '…') {
            continue;
        }

        let next_index = index + current.len_utf8();
        let next_char = text[next_index..].chars().next();
        if next_char.map(|c| !c.is_whitespace()).unwrap_or(false) {
            continue;
        }

        let sentence = text[sentence_start..next_index].trim();
        if !sentence.is_empty() {
            let relative_start = text[sentence_start..]
                .find(sentence)
                .map(|offset| sentence_start + offset)
                .unwrap_or(sentence_start);
            let start = base_offset + relative_start;
            let end = start + sentence.len();
            fragments.push((sentence.to_string(), start, end));
        }

        sentence_start = next_index;
    }

    let tail = text[sentence_start..].trim();
    if !tail.is_empty() {
        let relative_start = text[sentence_start..]
            .find(tail)
            .map(|offset| sentence_start + offset)
            .unwrap_or(sentence_start);
        let start = base_offset + relative_start;
        let end = start + tail.len();
        fragments.push((tail.to_string(), start, end));
    }

    if fragments.is_empty() {
        vec![(text.to_string(), base_offset, base_offset + text.len())]
    } else {
        fragments
    }
}

fn ends_at_sentence_boundary(text: &str) -> bool {
    matches!(
        text.trim_end().chars().last(),
        Some('.' | '!' | '?' | '…')
    )
}

fn merge_fragments(
    fragments: Vec<(String, usize, usize)>,
    target: usize,
) -> Vec<(String, usize, usize)> {
    // Tolerate chunks up to hard_max to avoid mid-sentence cuts while staying
    // within Kokoro's ~505-phoneme budget. Italian/English IPA expansion is
    // roughly 1.2–1.5× the character count (stress marks, digraphs), so 330
    // chars ≈ 500 phonemes at worst. Keeping the ceiling tight: going higher
    // triggers the phoneme guard in tts-mlx and the chunk is silently dropped.
    let hard_max = target.saturating_add(target / 10).min(330);

    let mut merged = Vec::new();
    let mut buffer_texts: Vec<String> = Vec::new();
    let mut buffer_start = 0usize;
    let mut buffer_end = 0usize;
    let mut buffer_len = 0usize;

    for (text, start, end) in fragments {
        let addition = text.chars().count() + if buffer_texts.is_empty() { 0 } else { 1 };

        if !buffer_texts.is_empty() && buffer_len + addition > target {
            let buffer_at_boundary = buffer_texts
                .last()
                .map(|t| ends_at_sentence_boundary(t))
                .unwrap_or(false);
            let fragment_at_boundary = ends_at_sentence_boundary(&text);

            if buffer_at_boundary {
                // Natural sentence boundary — flush the buffer here.
                merged.push((buffer_texts.join(" "), buffer_start, buffer_end));
                buffer_texts = vec![text];
                buffer_start = start;
                buffer_end = end;
                buffer_len = buffer_texts[0].chars().count();
            } else if fragment_at_boundary && buffer_len + addition <= hard_max {
                // Buffer doesn't end at a boundary, but this fragment completes
                // the sentence and fits within hard_max. Add it then flush.
                buffer_end = end;
                buffer_texts.push(text);
                merged.push((buffer_texts.join(" "), buffer_start, buffer_end));
                buffer_texts = Vec::new();
                buffer_len = 0;
            } else {
                // Hard cut — no nearby sentence boundary within phoneme budget.
                merged.push((buffer_texts.join(" "), buffer_start, buffer_end));
                buffer_texts = vec![text];
                buffer_start = start;
                buffer_end = end;
                buffer_len = buffer_texts[0].chars().count();
            }
            continue;
        }

        if buffer_texts.is_empty() {
            buffer_start = start;
        }
        buffer_len += addition;
        buffer_end = end;
        buffer_texts.push(text);
    }

    if !buffer_texts.is_empty() {
        merged.push((buffer_texts.join(" "), buffer_start, buffer_end));
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::{
        build_document_from_import, Document, DocumentChunk, DocumentSection, ImportedDocument,
        ImportedSection, DEFAULT_CHUNK_TARGET_CHARS,
    };
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
        assert_eq!(
            section.get_chunk(1).map(|chunk| chunk.anchor()),
            Some("chunk:1".to_string())
        );
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
            document.get_chunk(1, 1).map(|chunk| chunk.text.as_str()),
            Some("Gamma")
        );
    }

    #[test]
    fn imported_document_canonical_text_joins_titles_and_paragraphs() {
        let imported = ImportedDocument {
            title: Some("Doc".to_string()),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![ImportedSection {
                title: "Intro".to_string(),
                paragraphs: vec!["Alpha".to_string(), "Beta".to_string()],
                source_anchor: Some("section:0".to_string()),
            }],
        };

        assert_eq!(imported.canonical_text(), "Intro\n\nAlpha\n\nBeta");
    }

    #[test]
    fn build_document_from_import_keeps_section_structure_and_chunking() {
        let imported = ImportedDocument {
            title: Some("Doc".to_string()),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![
                ImportedSection {
                    title: "Intro".to_string(),
                    paragraphs: vec![
                        "Alpha beta gamma.".to_string(),
                        "Delta epsilon zeta.".to_string(),
                    ],
                    source_anchor: Some("section:0".to_string()),
                },
                ImportedSection {
                    title: String::new(),
                    paragraphs: vec!["Eta theta iota.".to_string()],
                    source_anchor: None,
                },
            ],
        };

        let document = build_document_from_import(imported, DEFAULT_CHUNK_TARGET_CHARS);

        assert_eq!(document.title, "Doc");
        assert_eq!(document.chapter_count(), 2);
        assert_eq!(document.sections[0].title, "Intro");
        assert_eq!(document.sections[1].title, "Section 2");
        assert_eq!(
            document.sections[1].source_anchor.as_deref(),
            Some("section:1")
        );
        assert_eq!(
            document.sections[0].chunks[0].text,
            "Alpha beta gamma. Delta epsilon zeta."
        );
    }
}
