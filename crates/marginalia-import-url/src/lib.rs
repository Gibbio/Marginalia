//! URL importer for Marginalia.
//!
//! Fetches a web page over HTTP(S), runs it through Mozilla's Readability
//! algorithm (via `readability-rust`) to strip navigation / ads / sidebars,
//! and extracts block-level paragraphs from the cleaned article HTML.
//!
//! Short URLs (`bit.ly`, `t.co`, `ow.ly`, etc.) resolve transparently because
//! `ureq` follows up to 5 redirects by default and preserves the final URL
//! on the response.
//!
//! Unlike `DocumentImporter`, this crate does not implement the port trait:
//! URLs are not paths, and keeping the command surface distinct (`/ingest`
//! vs `/ingest_url`) keeps the runtime routing explicit.

use marginalia_core::domain::{ImportedDocument, ImportedSection};
use marginalia_core::ports::DocumentImportError;
use readability_rust::Readability;
use scraper::{Html, Selector};
use std::path::PathBuf;
use std::time::Duration;

/// Minimum length of an extracted paragraph to keep. Filters out single-word
/// captions, share buttons, and stray inline markup.
const MIN_BLOCK_LEN: usize = 15;

/// Upper bound on the size of a fetched page. Protects against OOM on
/// pathological URLs (infinite HTML streams, accidental binary downloads).
/// `ureq` applies the limit during `into_string`.
const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// User-Agent header. Identifies Marginalia so site operators can see traffic.
const USER_AGENT: &str =
    concat!("Marginalia/", env!("CARGO_PKG_VERSION"), " (reader; +https://github.com/Gibbio/Marginalia)");

pub struct UrlDocumentImporter {
    agent: ureq::Agent,
}

impl UrlDocumentImporter {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(30))
            .timeout_read(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .redirects(5)
            .build();
        Self { agent }
    }

    /// Fetch `url`, extract the readable article, and return an
    /// `ImportedDocument` ready to be chunked and synthesized.
    pub fn import_url(&self, url: &str) -> Result<ImportedDocument, DocumentImportError> {
        let parsed = url::Url::parse(url).map_err(|e| read_failed(url, format!("invalid URL: {e}")))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(read_failed(
                url,
                format!("unsupported scheme: {} (only http/https)", parsed.scheme()),
            ));
        }

        log::info!("url-import: fetching {url}");
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| read_failed(url, format!("HTTP request failed: {e}")))?;

        // ureq 2.x follows redirects transparently; `response.get_url()` is
        // the final URL the payload came from, useful for logging short URLs
        // and for the persisted source_path.
        let final_url = response.get_url().to_string();
        if final_url != url {
            log::info!("url-import: redirected to {final_url}");
        }

        let status = response.status();
        if !(200..300).contains(&status) {
            return Err(read_failed(
                url,
                format!("HTTP {status} {}", response.status_text()),
            ));
        }

        let html = response
            .into_string()
            .map_err(|e| read_failed(url, format!("failed to read response body: {e}")))?;

        if html.len() > MAX_RESPONSE_BYTES {
            // into_string caps at 10 MB by default, but we check again in
            // case a future ureq bumps the default.
            return Err(read_failed(
                url,
                format!("response body too large ({} bytes)", html.len()),
            ));
        }

        let mut parser = Readability::new(&html, None)
            .map_err(|e| read_failed(url, format!("readability init failed: {e:?}")))?;
        let article = parser.parse().ok_or_else(|| DocumentImportError::EmptyContent {
            source_path: PathBuf::from(&final_url),
        })?;

        let cleaned_html = article.content.clone().ok_or_else(|| {
            DocumentImportError::EmptyContent {
                source_path: PathBuf::from(&final_url),
            }
        })?;

        let paragraphs = extract_paragraphs(&cleaned_html);
        if paragraphs.is_empty() {
            return Err(DocumentImportError::EmptyContent {
                source_path: PathBuf::from(&final_url),
            });
        }

        let title = article
            .title
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| parsed.host_str().map(String::from));

        let section_title = title.clone().unwrap_or_else(|| "Article".to_string());

        Ok(ImportedDocument {
            title,
            // Persist the final URL (after redirects) so cache / dedup keys
            // are stable on re-ingest of a short URL.
            source_path: PathBuf::from(&final_url),
            sections: vec![ImportedSection {
                title: section_title,
                paragraphs,
                source_anchor: Some(format!("url:{final_url}")),
            }],
        })
    }
}

impl Default for UrlDocumentImporter {
    fn default() -> Self {
        Self::new()
    }
}

fn read_failed(url: &str, message: String) -> DocumentImportError {
    DocumentImportError::ReadFailed {
        source_path: PathBuf::from(url),
        message,
    }
}

/// Extract plain-text paragraphs from the cleaned article HTML returned by
/// readability. Same block-level CSS selector used for EPUBs — readability's
/// output is semantically clean HTML, so the approach translates 1:1.
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

    // Fallback for pathological HTML where no block elements matched.
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
    fn rejects_non_http_scheme() {
        let importer = UrlDocumentImporter::new();
        let err = importer.import_url("file:///etc/passwd").unwrap_err();
        match err {
            DocumentImportError::ReadFailed { message, .. } => {
                assert!(message.contains("unsupported scheme"), "got {message}");
            }
            other => panic!("expected ReadFailed, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_url() {
        let importer = UrlDocumentImporter::new();
        let err = importer.import_url("not a url").unwrap_err();
        match err {
            DocumentImportError::ReadFailed { message, .. } => {
                assert!(message.contains("invalid URL"), "got {message}");
            }
            other => panic!("expected ReadFailed, got {other:?}"),
        }
    }

    #[test]
    fn extract_paragraphs_picks_block_elements() {
        let html = r#"
            <html><body>
              <article>
                <h1>An Excellent Article Title From The Web</h1>
                <p>This is the first meaningful paragraph in the cleaned article.</p>
                <p>tiny</p>
                <p>Second meaningful paragraph with enough content for the filter.</p>
              </article>
            </body></html>
        "#;
        let paragraphs = extract_paragraphs(html);
        assert_eq!(paragraphs.len(), 3, "got {paragraphs:?}");
    }

    #[test]
    fn collapse_whitespace_keeps_single_spaces() {
        assert_eq!(
            collapse_whitespace("hello\t\nworld   foo"),
            "hello world foo"
        );
    }
}
