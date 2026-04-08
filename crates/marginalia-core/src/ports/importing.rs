use crate::domain::ImportedDocument;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentImportError {
    EmptyContent { source_path: PathBuf },
    ReadFailed { source_path: PathBuf, message: String },
    UnsupportedFormat { source_path: PathBuf },
}

impl Display for DocumentImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyContent { source_path } => {
                write!(f, "The source file is empty: {}", source_path.display())
            }
            Self::ReadFailed {
                source_path,
                message,
            } => write!(f, "Failed to read {}: {}", source_path.display(), message),
            Self::UnsupportedFormat { source_path } => write!(
                f,
                "Unsupported document format for {}.",
                source_path.display()
            ),
        }
    }
}

impl Error for DocumentImportError {}

pub trait DocumentImporter {
    fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError>;
}

impl<T> DocumentImporter for &T
where
    T: DocumentImporter + ?Sized,
{
    fn import_path(&self, source_path: &Path) -> Result<ImportedDocument, DocumentImportError> {
        (**self).import_path(source_path)
    }
}
