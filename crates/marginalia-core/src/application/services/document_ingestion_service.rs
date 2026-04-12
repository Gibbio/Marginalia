use crate::domain::{build_document_from_import, Document};
use crate::events::{DomainEvent, EventName};
use crate::ports::events::EventPublisher;
use crate::ports::storage::{DocumentRepository, StorageError};
use crate::ports::{DocumentImportError, DocumentImporter};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(1);

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum IngestionError {
    Import(DocumentImportError),
    Storage(StorageError),
}

impl Display for IngestionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Import(e) => write!(f, "{e}"),
            Self::Storage(e) => write!(f, "storage error during ingestion: {e}"),
        }
    }
}

impl std::error::Error for IngestionError {}

impl From<DocumentImportError> for IngestionError {
    fn from(e: DocumentImportError) -> Self {
        Self::Import(e)
    }
}

impl From<StorageError> for IngestionError {
    fn from(e: StorageError) -> Self {
        Self::Storage(e)
    }
}

// ---------------------------------------------------------------------------
// Outcome
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentIngestionStats {
    pub raw_char_count: usize,
    pub chapter_count: usize,
    pub chunk_count: usize,
    pub average_chunk_chars: f64,
    pub max_chunk_chars: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentIngestionOutcome {
    pub document: Document,
    pub already_present: bool,
    pub stats: DocumentIngestionStats,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct DocumentIngestionService<R, I, E>
where
    R: DocumentRepository,
    I: DocumentImporter,
    E: EventPublisher,
{
    document_repository: R,
    importer: I,
    event_publisher: E,
    chunk_target_chars: usize,
}

impl<R, I, E> DocumentIngestionService<R, I, E>
where
    R: DocumentRepository,
    I: DocumentImporter,
    E: EventPublisher,
{
    pub fn new(
        document_repository: R,
        importer: I,
        event_publisher: E,
        chunk_target_chars: usize,
    ) -> Self {
        Self {
            document_repository,
            importer,
            event_publisher,
            chunk_target_chars: chunk_target_chars.max(1),
        }
    }

    pub fn ingest_path(
        &mut self,
        source_path: &Path,
    ) -> Result<DocumentIngestionOutcome, IngestionError> {
        let imported = self.importer.import_path(source_path)?;
        let raw_char_count = imported.canonical_text().chars().count();
        let document = build_document_from_import(imported, self.chunk_target_chars);
        let already_present = self
            .document_repository
            .get_document(&document.document_id)
            .is_some();
        self.document_repository.save_document(document.clone())?;

        let chunk_lengths = document
            .sections
            .iter()
            .flat_map(|section| section.chunks.iter())
            .map(|chunk| chunk.text.chars().count())
            .collect::<Vec<_>>();
        let stats = DocumentIngestionStats {
            raw_char_count,
            chapter_count: document.chapter_count(),
            chunk_count: document.total_chunk_count(),
            average_chunk_chars: if chunk_lengths.is_empty() {
                0.0
            } else {
                chunk_lengths.iter().sum::<usize>() as f64 / chunk_lengths.len() as f64
            },
            max_chunk_chars: chunk_lengths.iter().copied().max().unwrap_or(0),
        };

        self.publish_document_ingested(&document, already_present, &stats);

        Ok(DocumentIngestionOutcome {
            document,
            already_present,
            stats,
        })
    }

    fn publish_document_ingested(
        &self,
        document: &Document,
        already_present: bool,
        stats: &DocumentIngestionStats,
    ) {
        let mut payload = HashMap::new();
        payload.insert("document_id".to_string(), document.document_id.clone());
        payload.insert("title".to_string(), document.title.clone());
        payload.insert("chapter_count".to_string(), stats.chapter_count.to_string());
        payload.insert("chunk_count".to_string(), stats.chunk_count.to_string());
        payload.insert("already_present".to_string(), already_present.to_string());
        payload.insert(
            "raw_char_count".to_string(),
            stats.raw_char_count.to_string(),
        );

        self.event_publisher.publish(DomainEvent {
            name: EventName::DocumentIngested,
            payload,
            event_id: format!("event-{}", EVENT_COUNTER.fetch_add(1, Ordering::Relaxed)),
            occurred_at: chrono::Utc::now(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{DocumentIngestionService, IngestionError};
    use crate::domain::{ImportedDocument, ImportedSection, SearchQuery, SearchResult};
    use crate::events::{DomainEvent, EventName};
    use crate::ports::events::EventPublisher;
    use crate::ports::storage::{DocumentRepository, StorageError};
    use crate::ports::{DocumentImportError, DocumentImporter};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    struct StubImporter {
        document: Option<ImportedDocument>,
        error: Option<DocumentImportError>,
    }

    impl DocumentImporter for StubImporter {
        fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError> {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }

            self.document
                .clone()
                .map(|mut document| {
                    document.source_path = source_path.to_path_buf();
                    document
                })
                .ok_or_else(|| DocumentImportError::UnsupportedFormat {
                    source_path: source_path.to_path_buf(),
                })
        }
    }

    #[derive(Default)]
    struct StubDocumentRepository {
        documents: Vec<crate::domain::Document>,
    }

    impl DocumentRepository for StubDocumentRepository {
        fn save_document(&mut self, document: crate::domain::Document) -> Result<(), StorageError> {
            self.documents
                .retain(|existing| existing.document_id != document.document_id);
            self.documents.push(document);
            Ok(())
        }

        fn get_document(&self, document_id: &str) -> Option<crate::domain::Document> {
            self.documents
                .iter()
                .find(|document| document.document_id == document_id)
                .cloned()
        }

        fn list_documents(&self) -> Vec<crate::domain::Document> {
            self.documents.clone()
        }

        fn search_documents(&self, _query: &SearchQuery) -> Vec<SearchResult> {
            Vec::new()
        }
    }

    #[derive(Default)]
    struct StubEventPublisher {
        events: RefCell<Vec<DomainEvent>>,
    }

    impl EventPublisher for StubEventPublisher {
        fn publish(&self, event: DomainEvent) {
            self.events.borrow_mut().push(event);
        }
    }

    fn imported_document() -> ImportedDocument {
        ImportedDocument {
            title: Some("Doc".to_string()),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![ImportedSection {
                title: "Intro".to_string(),
                paragraphs: vec![
                    "Alpha beta gamma.".to_string(),
                    "Delta epsilon zeta.".to_string(),
                ],
                source_anchor: Some("section:0".to_string()),
            }],
        }
    }

    #[test]
    fn ingest_path_saves_document_and_publishes_event() {
        let publisher = StubEventPublisher::default();
        let mut service = DocumentIngestionService::new(
            StubDocumentRepository::default(),
            StubImporter {
                document: Some(imported_document()),
                error: None,
            },
            publisher,
            300,
        );

        let outcome = service.ingest_path(Path::new("/tmp/doc.md")).unwrap();

        assert_eq!(outcome.document.title, "Doc");
        assert_eq!(outcome.stats.chapter_count, 1);
        assert_eq!(outcome.stats.chunk_count, 1);
        assert!(!outcome.already_present);
    }

    #[test]
    fn ingest_path_returns_import_error() {
        let mut service = DocumentIngestionService::new(
            StubDocumentRepository::default(),
            StubImporter {
                document: None,
                error: Some(DocumentImportError::EmptyContent {
                    source_path: PathBuf::from("/tmp/empty.md"),
                }),
            },
            StubEventPublisher::default(),
            300,
        );

        let error = service.ingest_path(Path::new("/tmp/empty.md")).unwrap_err();

        assert_eq!(
            error,
            IngestionError::Import(DocumentImportError::EmptyContent {
                source_path: PathBuf::from("/tmp/empty.md"),
            })
        );
    }

    #[test]
    fn ingest_path_marks_document_as_already_present() {
        let imported = imported_document();
        let existing = crate::domain::build_document_from_import(imported.clone(), 300);
        let mut service = DocumentIngestionService::new(
            StubDocumentRepository {
                documents: vec![existing],
            },
            StubImporter {
                document: Some(imported),
                error: None,
            },
            StubEventPublisher::default(),
            300,
        );

        let outcome = service.ingest_path(Path::new("/tmp/doc.md")).unwrap();

        assert!(outcome.already_present);
    }

    #[test]
    fn published_event_uses_document_ingested_name() {
        let publisher = StubEventPublisher::default();
        let mut service = DocumentIngestionService::new(
            StubDocumentRepository::default(),
            StubImporter {
                document: Some(imported_document()),
                error: None,
            },
            publisher,
            300,
        );

        let _ = service.ingest_path(Path::new("/tmp/doc.md")).unwrap();

        let events = service.event_publisher.events.borrow();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, EventName::DocumentIngested);
        assert_eq!(
            events[0].payload.get("title").map(String::as_str),
            Some("Doc")
        );
    }
}
