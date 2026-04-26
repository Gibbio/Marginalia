ALTER TABLE documents ADD COLUMN content_sha256 TEXT;
ALTER TABLE documents ADD COLUMN content_size_bytes INTEGER;
ALTER TABLE documents ADD COLUMN content_mtime_unix_ms INTEGER;
CREATE INDEX IF NOT EXISTS idx_documents_source_path ON documents(source_path);
