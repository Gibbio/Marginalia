# marginalia-import-url

Web article importer. Fetches a URL over HTTP(S), runs it through Mozilla
Readability, and exposes a single-section `ImportedDocument` ready for
chunking and TTS.

Invoked in the TUI as `/ingest_url <url>` (separate from `/ingest`, which
is for local files).

## Pipeline

1. Validate the URL with the `url` crate; reject non-HTTP(S) schemes.
2. Fetch with `ureq` — pure-Rust TLS via rustls, gzip enabled.
   - Timeouts: 30 s connect + 30 s read.
   - Redirects: follows up to 5, so short URLs (`bit.ly`, `t.co`, …)
     resolve transparently.
   - User-Agent: `Marginalia/<version> (reader; …)` — identifies the
     client to site operators.
   - Body size: capped at 10 MB to protect against pathological pages.
3. Run `readability-rust` (Rust port of Mozilla Readability.js) against the
   HTML to pull out the readable article and strip navigation / ads /
   sidebars. Returns an `Article` with `title`, cleaned `content` (HTML),
   `text_content`, `byline`, `lang`, `site_name`, etc.
4. Feed `Article.content` (cleaned HTML) through `scraper` with the same
   block-level selector used by the EPUB importer
   (`p, h1..h6, li, blockquote`). Paragraphs shorter than 15 characters
   are dropped.
5. Produce an `ImportedDocument` with one section named after the article
   title (fallback: hostname). The `source_path` carries the final URL
   after redirects — stable across re-ingestion of short URLs.

## Not handled

- Paywalls / JavaScript-rendered content. Readability only sees the HTML
  the server returns to a plain HTTP client; Single-Page Apps that render
  in the browser won't work.
- Authentication (cookies, headers, OAuth).
- Conditional GET / ETag caching.
