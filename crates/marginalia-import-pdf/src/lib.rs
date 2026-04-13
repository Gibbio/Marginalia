//! PDF importer for Marginalia.
//!
//! Implements `DocumentImporter` using PDFium (Google's PDF engine, same as Chrome).
//! Text is extracted page by page; each page becomes one `ImportedSection`.
//!
//! Requires `libpdfium.dylib` (macOS) / `libpdfium.so` (Linux) at runtime.
//! Run `make bootstrap-pdf` to download the matching binary.
//!
//! Scanned PDFs (image-only) produce empty pages — OCR is out of scope.

use marginalia_core::domain::{ImportedDocument, ImportedSection};
use marginalia_core::ports::{DocumentImportError, DocumentImporter};
use pdfium_render::prelude::*;
use std::path::Path;

pub struct PdfDocumentImporter {
    pdfium: Pdfium,
}

impl PdfDocumentImporter {
    /// Try to load PDFium.
    ///
    /// Search order:
    ///   1. `models/pdf/lib/` — downloaded by `make bootstrap-pdf`
    ///   2. System library path (LD_LIBRARY_PATH / DYLD_LIBRARY_PATH)
    ///
    /// Returns `Err` with a human-readable message if PDFium is not found.
    /// Try to load PDFium from an explicit library directory.
    ///
    /// `lib_dir` should be an absolute path in production (e.g. next to the
    /// installed binary). In development, a relative path like `"models/pdf/lib"`
    /// works when the binary is run from the repo root.
    pub fn try_new_at(lib_dir: &std::path::Path) -> Result<Self, String> {
        let candidate = Pdfium::pdfium_platform_library_name_at_path(lib_dir);
        let bindings = Pdfium::bind_to_library(candidate)
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| format!("PDFium not found in {}: {e}. Run: make bootstrap-pdf", lib_dir.display()))?;
        Ok(Self {
            pdfium: Pdfium::new(bindings),
        })
    }
}

impl DocumentImporter for PdfDocumentImporter {
    fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError> {
        let doc = self
            .pdfium
            .load_pdf_from_file(source_path, None)
            .map_err(|e| DocumentImportError::ReadFailed {
                source_path: source_path.to_path_buf(),
                message: e.to_string(),
            })?;

        let mut sections = Vec::new();

        for (i, page) in doc.pages().iter().enumerate() {
            let raw = match page.text() {
                Ok(t) => t.all(),
                Err(e) => {
                    log::warn!("PDF page {}: text extraction failed: {e}", i + 1);
                    continue;
                }
            };
            let paragraphs = extract_paragraphs(&raw);

            if paragraphs.is_empty() {
                continue;
            }

            sections.push(ImportedSection {
                title: format!("Page {}", i + 1),
                paragraphs,
                source_anchor: Some(format!("page:{}", i + 1)),
            });
        }

        if sections.is_empty() {
            return Err(DocumentImportError::EmptyContent {
                source_path: source_path.to_path_buf(),
            });
        }

        let title = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(String::from);

        Ok(ImportedDocument {
            title,
            source_path: source_path.to_path_buf(),
            sections,
        })
    }
}

/// Minimum character length for a paragraph to be kept.
/// Shorter fragments are assumed to be page numbers, running headers, or rule lines.
const MIN_PARAGRAPH_LEN: usize = 15;

/// Split raw PDF page text into clean paragraphs ready for TTS chunking.
///
/// PDF text from PDFium uses `\n` for line endings within a text block and
/// `\n\n` (or more) for paragraph breaks. Hyphenated line breaks (`word-\n`)
/// are re-joined into the full word.
fn extract_paragraphs(raw: &str) -> Vec<String> {
    // Re-join soft hyphens: "word-\nword" → "wordword"
    let dehyphenated = raw.replace("-\n", "");

    // Split on paragraph boundaries (two or more newlines)
    dehyphenated
        .split("\n\n")
        .map(|block| {
            // Collapse inline newlines and whitespace within a paragraph
            block
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|p| p.len() > MIN_PARAGRAPH_LEN)
        .collect()
}
