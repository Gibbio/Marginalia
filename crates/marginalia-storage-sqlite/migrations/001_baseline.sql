CREATE TABLE IF NOT EXISTS schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS documents (
    document_id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    source_path TEXT NOT NULL,
    imported_at TEXT NOT NULL,
    outline_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS document_sections (
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    title TEXT NOT NULL,
    source_anchor TEXT,
    PRIMARY KEY(document_id, section_index)
);

CREATE TABLE IF NOT EXISTS document_chunks (
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    anchor TEXT NOT NULL,
    text TEXT NOT NULL,
    char_start INTEGER NOT NULL,
    char_end INTEGER NOT NULL,
    PRIMARY KEY(document_id, section_index, chunk_index)
);

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    state TEXT NOT NULL,
    playback_state TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    char_offset INTEGER NOT NULL,
    active_note_id TEXT,
    last_command TEXT,
    last_command_source TEXT,
    last_recognized_command TEXT,
    voice TEXT,
    tts_provider TEXT,
    command_stt_provider TEXT,
    playback_provider TEXT,
    command_listening_active INTEGER NOT NULL DEFAULT 0,
    command_language TEXT,
    audio_reference TEXT,
    playback_process_id INTEGER,
    runtime_process_id INTEGER,
    runtime_status TEXT,
    runtime_error TEXT,
    startup_cleanup_summary TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
    note_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    char_offset INTEGER NOT NULL,
    transcript TEXT NOT NULL,
    transcription_provider TEXT NOT NULL DEFAULT 'unknown',
    language TEXT NOT NULL DEFAULT 'en',
    raw_audio_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS drafts (
    draft_id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    section_index INTEGER NOT NULL,
    source_anchor TEXT NOT NULL DEFAULT 'section:0/chunk:0',
    source_excerpt TEXT NOT NULL,
    note_transcripts_json TEXT NOT NULL,
    rewritten_text TEXT NOT NULL,
    provider_name TEXT NOT NULL DEFAULT 'unknown',
    status TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_documents_imported_at ON documents(imported_at DESC);
CREATE INDEX IF NOT EXISTS idx_document_sections_document_id
    ON document_sections(document_id, section_index);
CREATE INDEX IF NOT EXISTS idx_document_chunks_document_id
    ON document_chunks(document_id, section_index, chunk_index);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_notes_document_id ON notes(document_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_drafts_document_id ON drafts(document_id, created_at DESC);
