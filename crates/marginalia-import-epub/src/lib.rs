//! EPUB importer for Marginalia.
//!
//! Parses the EPUB spine (reading order) and maps each spine item to an
//! `ImportedSection`. Chapter titles are pulled from the EPUB's TOC when the
//! TOC points at a spine path; otherwise a `Chapter N` fallback is used.
//!
//! Text extraction uses a `scraper`-based CSS selector that keeps block-level
//! content (`p`, `h1`–`h6`, `li`, `blockquote`) and drops scripts, styles,
//! and images. EPUB 2 and EPUB 3 are both supported via the `epub` crate.
//!
//! No native dependency — EPUB is a ZIP archive of XHTML. The crate is
//! pure-Rust and available on every supported platform.

use epub::doc::{EpubDoc, NavPoint};
use marginalia_core::domain::{ImportedDocument, ImportedSection};
use marginalia_core::ports::{DocumentImportError, DocumentImporter};
use scraper::{Html, Selector};
use std::path::{Path, PathBuf};

/// Minimum length of an extracted block to keep. Filters out stray single
/// characters, page numbers, and decorative dividers.
const MIN_BLOCK_LEN: usize = 15;

pub struct EpubDocumentImporter;

impl EpubDocumentImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EpubDocumentImporter {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentImporter for EpubDocumentImporter {
    fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError> {
        let mut doc = EpubDoc::new(source_path).map_err(|e| DocumentImportError::ReadFailed {
            source_path: source_path.to_path_buf(),
            message: format!("failed to open EPUB: {e}"),
        })?;

        let book_title = doc.get_title();
        let toc_titles = flatten_toc(&doc.toc);

        let num_chapters = doc.get_num_chapters();
        let mut sections: Vec<ImportedSection> = Vec::new();

        for idx in 0..num_chapters {
            if !doc.set_current_chapter(idx) {
                continue;
            }
            let (html, _mime) = match doc.get_current_str() {
                Some(content) => content,
                None => {
                    log::warn!("EPUB chapter {}: could not read content", idx + 1);
                    continue;
                }
            };
            let current_path = doc.get_current_path();

            let paragraphs = extract_paragraphs(&html);
            if paragraphs.is_empty() {
                continue;
            }

            let title = current_path
                .as_ref()
                .and_then(|p| {
                    toc_titles
                        .iter()
                        .find(|(path, _)| paths_match(path, p))
                        .map(|(_, label)| label.clone())
                })
                .unwrap_or_else(|| format!("Chapter {}", idx + 1));

            let anchor = current_path
                .as_ref()
                .and_then(|p| p.to_str())
                .map(|s| format!("epub:{s}"));

            sections.push(ImportedSection {
                title,
                paragraphs,
                source_anchor: anchor,
            });
        }

        if sections.is_empty() {
            return Err(DocumentImportError::EmptyContent {
                source_path: source_path.to_path_buf(),
            });
        }

        Ok(ImportedDocument {
            title: book_title,
            source_path: source_path.to_path_buf(),
            sections,
        })
    }
}

/// Walk the TOC tree and collect (content_path, label) pairs for every nav
/// point at every depth. Used to map spine items back to chapter titles.
fn flatten_toc(tree: &[NavPoint]) -> Vec<(PathBuf, String)> {
    fn walk(out: &mut Vec<(PathBuf, String)>, nodes: &[NavPoint]) {
        for node in nodes {
            let clean = strip_fragment(&node.content);
            let label = node.label.trim().to_string();
            if !label.is_empty() {
                out.push((clean, label));
            }
            walk(out, &node.children);
        }
    }
    let mut out = Vec::new();
    walk(&mut out, tree);
    out
}

/// Strip a `#fragment` suffix (anchor into a page) from an EPUB content path.
/// The spine path has no fragment, so matching requires the TOC path to be
/// cleaned first.
fn strip_fragment(p: &Path) -> PathBuf {
    if let Some(s) = p.to_str() {
        if let Some(idx) = s.rfind('#') {
            return PathBuf::from(&s[..idx]);
        }
    }
    p.to_path_buf()
}

/// TOC paths in some EPUBs are relative to the OPF's directory while spine
/// paths are absolute to the archive root. Compare by the filename-carrying
/// tail so both conventions match.
fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (a.file_name(), b.file_name()) {
        (Some(x), Some(y)) if x == y => {
            // Compare parent components as well to disambiguate duplicate
            // filenames across directories.
            let a_parent = a.parent().and_then(Path::file_name);
            let b_parent = b.parent().and_then(Path::file_name);
            match (a_parent, b_parent) {
                (Some(_), None) | (None, Some(_)) | (None, None) => true,
                (Some(ap), Some(bp)) => ap == bp,
            }
        }
        _ => false,
    }
}

/// Extract plain-text paragraphs from an XHTML chapter body.
///
/// Scans for block-level elements that typically hold prose and collects
/// their inner text. Scripts, styles, and images are automatically skipped
/// because they are not selected.
fn extract_paragraphs(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);

    let selector =
        Selector::parse("p, h1, h2, h3, h4, h5, h6, li, blockquote").expect("static selector");

    let mut result: Vec<String> = Vec::new();
    for el in doc.select(&selector) {
        let text: String = el.text().collect::<Vec<_>>().join(" ");
        let cleaned = collapse_whitespace(text.trim());
        if cleaned.chars().count() >= MIN_BLOCK_LEN {
            result.push(cleaned);
        }
    }

    // Fallback: no block elements matched (malformed XHTML or unusual layout).
    // Use the whole body text split by newlines.
    if result.is_empty() {
        let body_selector = Selector::parse("body").expect("static selector");
        for body in doc.select(&body_selector) {
            let text: String = body.text().collect::<Vec<_>>().join(" ");
            for line in text.split('\n') {
                let cleaned = collapse_whitespace(line.trim());
                if cleaned.chars().count() >= MIN_BLOCK_LEN {
                    result.push(cleaned);
                }
            }
        }
    }

    result
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapse_whitespace_keeps_single_spaces() {
        assert_eq!(
            collapse_whitespace("hello   world\n\t\tfoo"),
            "hello world foo"
        );
    }

    #[test]
    fn extract_paragraphs_picks_block_elements() {
        let html = r#"
            <html><body>
              <h1>The Opening Chapter: A Beginning</h1>
              <p>This is the first paragraph, long enough to survive the MIN_BLOCK_LEN filter.</p>
              <p>Short.</p>
              <p>Second long paragraph of the chapter with meaningful content.</p>
            </body></html>
        "#;
        let paragraphs = extract_paragraphs(html);
        assert_eq!(paragraphs.len(), 3, "got {paragraphs:?}");
        assert!(paragraphs[0].contains("Opening Chapter"));
        assert!(paragraphs[1].contains("first paragraph"));
        // "Short." is filtered out by MIN_BLOCK_LEN.
        assert!(paragraphs[2].contains("Second long paragraph"));
    }

    #[test]
    fn strip_fragment_removes_anchor() {
        assert_eq!(
            strip_fragment(Path::new("OEBPS/ch1.xhtml#p5")),
            PathBuf::from("OEBPS/ch1.xhtml")
        );
        assert_eq!(
            strip_fragment(Path::new("OEBPS/ch1.xhtml")),
            PathBuf::from("OEBPS/ch1.xhtml")
        );
    }

    #[test]
    fn paths_match_handles_relative_and_absolute() {
        assert!(paths_match(
            Path::new("OEBPS/ch1.xhtml"),
            Path::new("OEBPS/ch1.xhtml")
        ));
        assert!(paths_match(
            Path::new("ch1.xhtml"),
            Path::new("OEBPS/ch1.xhtml")
        ));
        assert!(!paths_match(Path::new("ch1.xhtml"), Path::new("ch2.xhtml")));
    }
}
