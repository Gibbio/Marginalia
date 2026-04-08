use marginalia_core::domain::{ImportedDocument, ImportedSection};
use marginalia_core::ports::{DocumentImportError, DocumentImporter};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy)]
pub struct TextDocumentImporter;

impl DocumentImporter for TextDocumentImporter {
    fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError> {
        let extension = source_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase());

        let raw_text = fs::read_to_string(source_path).map_err(|error| {
            DocumentImportError::ReadFailed {
                source_path: source_path.to_path_buf(),
                message: error.to_string(),
            }
        })?;

        if raw_text.trim().is_empty() {
            return Err(DocumentImportError::EmptyContent {
                source_path: source_path.to_path_buf(),
            });
        }

        match extension.as_deref() {
            Some("txt") => Ok(import_plain_text(source_path.to_path_buf(), &raw_text)),
            Some("md") | Some("markdown") => {
                Ok(import_markdown(source_path.to_path_buf(), &raw_text))
            }
            _ => Err(DocumentImportError::UnsupportedFormat {
                source_path: source_path.to_path_buf(),
            }),
        }
    }
}

fn import_plain_text(source_path: PathBuf, raw_text: &str) -> ImportedDocument {
    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_paragraphs: Vec<String> = Vec::new();

    for line in raw_text.lines() {
        if line.trim_start().starts_with('#') {
            flush_section(&mut sections, current_title.take(), &mut current_paragraphs);
            let heading = line.trim_start_matches('#').trim();
            current_title = Some(heading.to_string());
            continue;
        }

        if line.trim().is_empty() {
            if !current_paragraphs.last().map(|p| p.is_empty()).unwrap_or(false) {
                current_paragraphs.push(String::new());
            }
            continue;
        }

        if let Some(last) = current_paragraphs.last_mut() {
            if !last.is_empty() {
                last.push(' ');
                last.push_str(line.trim());
                continue;
            }
        }

        current_paragraphs.push(line.trim().to_string());
    }

    flush_section(&mut sections, current_title.take(), &mut current_paragraphs);

    ImportedDocument {
        title: None,
        source_path,
        sections,
    }
}

fn import_markdown(source_path: PathBuf, raw_text: &str) -> ImportedDocument {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(raw_text, options);

    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_paragraphs: Vec<String> = Vec::new();
    let mut block_text = String::new();
    let mut active_block: Option<BlockKind> = None;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_section(&mut sections, current_title.take(), &mut current_paragraphs);
                active_block = Some(BlockKind::Heading);
                block_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                let title = normalize_inline_text(&block_text);
                current_title = if title.is_empty() { None } else { Some(title) };
                active_block = None;
                block_text.clear();
            }
            Event::Start(Tag::Paragraph) => {
                active_block = Some(BlockKind::Paragraph);
                block_text.clear();
            }
            Event::End(TagEnd::Paragraph) => {
                let paragraph = normalize_inline_text(&block_text);
                if !paragraph.is_empty() {
                    current_paragraphs.push(paragraph);
                }
                active_block = None;
                block_text.clear();
            }
            Event::Text(text) | Event::Code(text) => {
                if active_block.is_some() {
                    block_text.push_str(text.as_ref());
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if active_block.is_some() {
                    block_text.push(' ');
                }
            }
            _ => {}
        }
    }

    flush_section(&mut sections, current_title.take(), &mut current_paragraphs);

    ImportedDocument {
        title: None,
        source_path,
        sections,
    }
}

fn flush_section(
    sections: &mut Vec<ImportedSection>,
    current_title: Option<String>,
    current_paragraphs: &mut Vec<String>,
) {
    let paragraphs = current_paragraphs
        .drain(..)
        .map(|paragraph| paragraph.trim().to_string())
        .filter(|paragraph| !paragraph.is_empty())
        .collect::<Vec<_>>();

    if current_title.as_ref().map(|title| title.trim().is_empty()).unwrap_or(true)
        && paragraphs.is_empty()
    {
        return;
    }

    sections.push(ImportedSection {
        title: current_title.unwrap_or_default(),
        paragraphs,
        source_anchor: Some(format!("section:{}", sections.len())),
    });
}

fn normalize_inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    Heading,
    Paragraph,
}

#[cfg(test)]
mod tests {
    use super::TextDocumentImporter;
    use marginalia_core::ports::{DocumentImportError, DocumentImporter};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(extension: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("marginalia-import-test-{}.{}", timestamp, extension))
    }

    #[test]
    fn imports_plain_text_with_hash_headings() {
        let path = temp_path("txt");
        fs::write(
            &path,
            "# Intro\n\nAlpha beta gamma.\nDelta epsilon.\n\n# Two\n\nZeta eta.",
        )
        .unwrap();

        let importer = TextDocumentImporter;
        let imported = importer.import_path(&path).unwrap();

        assert_eq!(imported.sections.len(), 2);
        assert_eq!(imported.sections[0].title, "Intro");
        assert_eq!(
            imported.sections[0].paragraphs[0],
            "Alpha beta gamma. Delta epsilon."
        );
        assert_eq!(imported.sections[1].title, "Two");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn imports_markdown_headings_and_paragraphs() {
        let path = temp_path("md");
        fs::write(
            &path,
            "# Intro\n\nAlpha *beta* gamma.\n\n```rust\nlet hidden = true;\n```\n\n## Two\n\nDelta `epsilon` zeta.",
        )
        .unwrap();

        let importer = TextDocumentImporter;
        let imported = importer.import_path(&path).unwrap();

        assert_eq!(imported.sections.len(), 2);
        assert_eq!(imported.sections[0].title, "Intro");
        assert_eq!(imported.sections[0].paragraphs, vec!["Alpha beta gamma.".to_string()]);
        assert_eq!(imported.sections[1].title, "Two");
        assert_eq!(
            imported.sections[1].paragraphs,
            vec!["Delta epsilon zeta.".to_string()]
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_empty_files() {
        let path = temp_path("md");
        fs::write(&path, "   \n\n").unwrap();

        let importer = TextDocumentImporter;
        let error = importer.import_path(&path).unwrap_err();

        assert_eq!(
            error,
            DocumentImportError::EmptyContent {
                source_path: path.clone(),
            }
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unsupported_extensions() {
        let path = temp_path("epub");
        fs::write(&path, "placeholder").unwrap();

        let importer = TextDocumentImporter;
        let error = importer.import_path(&path).unwrap_err();

        assert_eq!(
            error,
            DocumentImportError::UnsupportedFormat {
                source_path: path.clone(),
            }
        );

        let _ = fs::remove_file(path);
    }
}
