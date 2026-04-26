//! Cheap source-file fingerprinting for change detection.
//!
//! Returns SHA-256 + size + mtime in one pass. Used by:
//! - `DocumentIngestionService::ingest_path` to populate the
//!   `documents.content_*` columns.
//! - The runtime's library list builder to flag rows whose source file
//!   on disk no longer matches the stored hash (drives the "modificato
//!   dall'ultima importazione" badge in the GUI).
//!
//! Streams the file through the SHA-256 engine in 64 KiB chunks so EPUBs
//! and PDFs (tens of MB) don't slurp memory. Caps at 256 MiB — anything
//! larger returns `InvalidData`; the row is then treated as "unknown
//! state" by the change-detection scan and the user keeps using whatever
//! version was last ingested.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::SystemTime;

/// Maximum file size we'll fingerprint. Beyond this, return InvalidData
/// so the caller can degrade gracefully rather than spinning on a
/// multi-GB blob.
const MAX_FINGERPRINT_BYTES: u64 = 256 * 1024 * 1024;

const READ_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFingerprint {
    /// Lowercase hex SHA-256 of the file's bytes.
    pub sha256_hex: String,
    /// File size in bytes.
    pub size_bytes: u64,
    /// File mtime in Unix epoch milliseconds (signed because `SystemTime`
    /// pre-epoch values yield negative deltas; we don't expect them on
    /// modern filesystems but keep the type honest).
    pub mtime_ms: i64,
}

/// Compute a fingerprint for the file at `path`. Errors are I/O errors
/// (file missing, permission denied, file too big). Symlinks are followed
/// (`File::open` semantics).
pub fn file_fingerprint(path: &Path) -> io::Result<FileFingerprint> {
    let metadata = std::fs::metadata(path)?;
    let size_bytes = metadata.len();
    if size_bytes > MAX_FINGERPRINT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file {} exceeds fingerprint cap ({} > {} bytes)",
                path.display(),
                size_bytes,
                MAX_FINGERPRINT_BYTES
            ),
        ));
    }
    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; READ_CHUNK_BYTES];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    let sha256_hex = format!("{:x}", hasher.finalize());

    Ok(FileFingerprint {
        sha256_hex,
        size_bytes,
        mtime_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fingerprint_matches_known_sha() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        let mut f = File::create(&path).unwrap();
        f.write_all(b"hello\n").unwrap();
        drop(f);

        let fp = file_fingerprint(&path).unwrap();
        // sha256("hello\n") via shell: echo "hello" | shasum -a 256
        assert_eq!(
            fp.sha256_hex,
            "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
        );
        assert_eq!(fp.size_bytes, 6);
    }

    #[test]
    fn fingerprint_rejects_huge_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zero.bin");
        let f = File::create(&path).unwrap();
        f.set_len(MAX_FINGERPRINT_BYTES + 1).unwrap();
        drop(f);

        let err = file_fingerprint(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn fingerprint_missing_file_is_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let err = file_fingerprint(&dir.path().join("nope")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
