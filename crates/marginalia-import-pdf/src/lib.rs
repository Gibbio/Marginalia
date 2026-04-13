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

/// Maximum paragraph length passed to the ingestion pipeline.
/// Kokoro's token limit is 512; IPA expansion is roughly 1.5–2× character count.
/// 250 chars × 2 = 500 tokens — safely under the 512 limit.
const MAX_PARAGRAPH_LEN: usize = 250;

/// Split raw PDF page text into clean paragraphs ready for TTS chunking.
///
/// PDF text from PDFium uses `\n` for line endings within a text block and
/// `\n\n` (or more) for paragraph breaks. Hyphenated line breaks (`word-\n`)
/// are re-joined into the full word.
///
/// Paragraphs longer than MAX_PARAGRAPH_LEN are further split at sentence
/// boundaries (`. ! ?`) to keep each unit within Kokoro's token limit.
fn extract_paragraphs(raw: &str) -> Vec<String> {
    // Re-join soft hyphens: "word-\nword" → "wordword"
    let dehyphenated = raw.replace("-\n", "");

    // Split on paragraph boundaries (two or more newlines), then collapse
    // inline newlines within each paragraph.
    let coarse: Vec<String> = dehyphenated
        .split("\n\n")
        .map(|block| {
            block
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|p| p.len() > MIN_PARAGRAPH_LEN)
        .collect();

    // Further split paragraphs that are too long for the TTS engine.
    let mut result = Vec::new();
    for para in coarse {
        if para.len() <= MAX_PARAGRAPH_LEN {
            result.push(para);
        } else {
            result.extend(split_at_sentences(&para));
        }
    }
    result
}

/// Split a long paragraph at sentence boundaries (`. ! ?`), keeping each
/// piece under MAX_PARAGRAPH_LEN. Falls back to a hard cut if no boundary
/// is found within the limit.
fn split_at_sentences(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        let is_sentence_end = matches!(ch, '.' | '!' | '?' | '…');
        if is_sentence_end && current.len() >= MIN_PARAGRAPH_LEN {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                chunks.push(trimmed);
            }
            current.clear();
        } else if current.len() >= MAX_PARAGRAPH_LEN {
            // No sentence boundary found — hard cut at last space.
            if let Some(pos) = current.rfind(' ') {
                let head = current[..pos].trim().to_string();
                let tail = current[pos + 1..].to_string();
                if !head.is_empty() {
                    chunks.push(head);
                }
                current = tail;
            } else {
                chunks.push(current.trim().to_string());
                current.clear();
            }
        }
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}
