use marginalia_core::domain::{
    Document, DocumentChunk, DocumentSection, PlaybackState, ReaderState, ReadingPosition,
    ReadingSession, RewriteDraft, RewriteStatus, SearchQuery, SearchResult, VoiceNote,
};
use marginalia_core::ports::storage::{
    DocumentRepository, NoteRepository, RewriteDraftRepository, SessionRepository,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SQLiteDocumentRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteDocumentRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl DocumentRepository for SQLiteDocumentRepository {
    fn ensure_schema(&mut self) {}

    fn save_document(&mut self, document: Document) {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        connection
            .execute(
                "
                INSERT INTO documents(document_id, title, source_path, imported_at, outline_json)
                VALUES(?, ?, ?, ?, ?)
                ON CONFLICT(document_id) DO UPDATE SET
                    title = excluded.title,
                    source_path = excluded.source_path,
                    imported_at = excluded.imported_at,
                    outline_json = excluded.outline_json
                ",
                params![
                    document.document_id,
                    document.title,
                    document.source_path.to_string_lossy().to_string(),
                    document.imported_at.to_rfc3339(),
                    document_to_json(&document),
                ],
            )
            .unwrap();
        connection
            .execute("DELETE FROM document_chunks WHERE document_id = ?", params![document.document_id.clone()])
            .unwrap();
        connection
            .execute("DELETE FROM document_sections WHERE document_id = ?", params![document.document_id.clone()])
            .unwrap();

        for section in &document.sections {
            connection
                .execute(
                    "
                    INSERT INTO document_sections(document_id, section_index, title, source_anchor)
                    VALUES(?, ?, ?, ?)
                    ",
                    params![
                        document.document_id,
                        section.index as i64,
                        section.title,
                        section.source_anchor,
                    ],
                )
                .unwrap();

            for chunk in &section.chunks {
                connection
                    .execute(
                        "
                        INSERT INTO document_chunks(
                            document_id,
                            section_index,
                            chunk_index,
                            anchor,
                            text,
                            char_start,
                            char_end
                        )
                        VALUES(?, ?, ?, ?, ?, ?, ?)
                        ",
                        params![
                            document.document_id,
                            section.index as i64,
                            chunk.index as i64,
                            format!("section:{}/chunk:{}", section.index, chunk.index),
                            chunk.text,
                            chunk.char_start as i64,
                            chunk.char_end as i64,
                        ],
                    )
                    .unwrap();
            }
        }
    }

    fn get_document(&self, document_id: &str) -> Option<Document> {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let row = connection
            .query_row(
                "
                SELECT document_id, title, source_path, imported_at
                FROM documents
                WHERE document_id = ?
                ",
                params![document_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .unwrap()?;

        let mut sections_statement = connection
            .prepare(
                "
                SELECT section_index, title, source_anchor
                FROM document_sections
                WHERE document_id = ?
                ORDER BY section_index ASC
                ",
            )
            .unwrap();
        let section_rows = sections_statement
            .query_map(params![document_id], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .unwrap();

        let mut sections = Vec::new();
        for row in section_rows {
            let (section_index, title, source_anchor) = row.unwrap();
            let mut chunk_statement = connection
                .prepare(
                    "
                    SELECT chunk_index, text, char_start, char_end
                    FROM document_chunks
                    WHERE document_id = ? AND section_index = ?
                    ORDER BY chunk_index ASC
                    ",
                )
                .unwrap();
            let chunk_rows = chunk_statement
                .query_map(params![document_id, section_index as i64], |row| {
                    Ok(DocumentChunk {
                        index: row.get::<_, i64>(0)? as usize,
                        text: row.get::<_, String>(1)?,
                        char_start: row.get::<_, i64>(2)? as usize,
                        char_end: row.get::<_, i64>(3)? as usize,
                    })
                })
                .unwrap();
            let chunks = chunk_rows.map(|row| row.unwrap()).collect::<Vec<_>>();

            sections.push(DocumentSection {
                index: section_index,
                title,
                chunks,
                source_anchor,
            });
        }

        Some(Document {
            document_id: row.0,
            title: row.1,
            source_path: PathBuf::from(row.2),
            imported_at: chrono::DateTime::parse_from_rfc3339(&row.3)
                .unwrap()
                .with_timezone(&chrono::Utc),
            sections,
        })
    }

    fn list_documents(&self) -> Vec<Document> {
        let document_ids = {
            let connection = self.connection.lock().expect("sqlite connection lock poisoned");
            let mut statement = connection
                .prepare("SELECT document_id FROM documents ORDER BY imported_at DESC")
                .unwrap();
            let rows = statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap();

            rows.map(|row| row.unwrap()).collect::<Vec<_>>()
        };

        document_ids
            .into_iter()
            .filter_map(|document_id| self.get_document(&document_id))
            .collect()
    }

    fn search_documents(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let needle = format!("%{}%", query.normalized_text().to_lowercase());
        if needle == "%%" {
            return Vec::new();
        }

        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let mut sql = "
            SELECT
                d.document_id,
                c.text,
                c.anchor
            FROM document_chunks c
            JOIN documents d ON d.document_id = c.document_id
            WHERE LOWER(c.text) LIKE ?
        "
        .to_string();

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(needle)];
        if let Some(document_id) = &query.document_id {
            sql.push_str(" AND d.document_id = ?");
            params_vec.push(Box::new(document_id.clone()));
        }
        sql.push_str(" ORDER BY d.imported_at DESC, c.section_index ASC, c.chunk_index ASC LIMIT ?");
        params_vec.push(Box::new(query.limit.max(1) as i64));

        let mut statement = connection.prepare(&sql).unwrap();
        let params_ref = params_vec.iter().map(|value| value.as_ref()).collect::<Vec<_>>();
        let rows = statement
            .query_map(rusqlite::params_from_iter(params_ref), |row| {
                Ok(SearchResult {
                    entity_kind: "document".to_string(),
                    entity_id: row.get::<_, String>(0)?,
                    score: 1.0,
                    excerpt: row.get::<_, String>(1)?,
                    anchor: row.get::<_, String>(2)?,
                })
            })
            .unwrap();

        rows.map(|row| row.unwrap()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct SQLiteSessionRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteSessionRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl SessionRepository for SQLiteSessionRepository {
    fn ensure_schema(&mut self) {}

    fn save_session(&mut self, session: ReadingSession) {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        if session.is_active {
            connection
                .execute(
                    "UPDATE sessions SET is_active = 0 WHERE session_id != ? AND is_active = 1",
                    params![session.session_id.clone()],
                )
                .unwrap();
        }

        connection
            .execute(
                "
                INSERT INTO sessions(
                    session_id,
                    document_id,
                    state,
                    playback_state,
                    section_index,
                    chunk_index,
                    char_offset,
                    active_note_id,
                    last_command,
                    last_command_source,
                    last_recognized_command,
                    voice,
                    tts_provider,
                    command_stt_provider,
                    playback_provider,
                    command_listening_active,
                    command_language,
                    audio_reference,
                    playback_process_id,
                    runtime_process_id,
                    runtime_status,
                    runtime_error,
                    startup_cleanup_summary,
                    is_active,
                    updated_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    state = excluded.state,
                    playback_state = excluded.playback_state,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    active_note_id = excluded.active_note_id,
                    last_command = excluded.last_command,
                    last_command_source = excluded.last_command_source,
                    last_recognized_command = excluded.last_recognized_command,
                    voice = excluded.voice,
                    tts_provider = excluded.tts_provider,
                    command_stt_provider = excluded.command_stt_provider,
                    playback_provider = excluded.playback_provider,
                    command_listening_active = excluded.command_listening_active,
                    command_language = excluded.command_language,
                    audio_reference = excluded.audio_reference,
                    playback_process_id = excluded.playback_process_id,
                    runtime_process_id = excluded.runtime_process_id,
                    runtime_status = excluded.runtime_status,
                    runtime_error = excluded.runtime_error,
                    startup_cleanup_summary = excluded.startup_cleanup_summary,
                    is_active = excluded.is_active,
                    updated_at = excluded.updated_at
                ",
                params![
                    session.session_id,
                    session.document_id,
                    reader_state_to_str(session.state),
                    playback_state_to_str(session.playback_state),
                    session.position.section_index as i64,
                    session.position.chunk_index as i64,
                    session.position.char_offset as i64,
                    session.active_note_id,
                    session.last_command,
                    session.last_command_source,
                    session.last_recognized_command,
                    session.voice,
                    session.tts_provider,
                    session.command_stt_provider,
                    session.playback_provider,
                    if session.command_listening_active { 1 } else { 0 },
                    session.command_language,
                    session.audio_reference,
                    session.playback_process_id.map(|v| v as i64),
                    session.runtime_process_id.map(|v| v as i64),
                    session.runtime_status,
                    session.runtime_error,
                    session.startup_cleanup_summary,
                    if session.is_active { 1 } else { 0 },
                    session.updated_at.to_rfc3339(),
                ],
            )
            .unwrap();
    }

    fn get_active_session(&self) -> Option<ReadingSession> {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        connection
            .query_row(
                "
                SELECT *
                FROM sessions
                WHERE is_active = 1
                ORDER BY updated_at DESC
                LIMIT 1
                ",
                [],
                |row| {
                    Ok(ReadingSession {
                        session_id: row.get::<_, String>("session_id")?,
                        document_id: row.get::<_, String>("document_id")?,
                        state: reader_state_from_str(&row.get::<_, String>("state")?),
                        playback_state: playback_state_from_str(
                            &row.get::<_, String>("playback_state")?,
                        ),
                        position: ReadingPosition {
                            section_index: row.get::<_, i64>("section_index")? as usize,
                            chunk_index: row.get::<_, i64>("chunk_index")? as usize,
                            char_offset: row.get::<_, i64>("char_offset")? as usize,
                        },
                        active_note_id: row.get::<_, Option<String>>("active_note_id")?,
                        last_command: row.get::<_, Option<String>>("last_command")?,
                        last_command_source: row
                            .get::<_, Option<String>>("last_command_source")?,
                        last_recognized_command: row
                            .get::<_, Option<String>>("last_recognized_command")?,
                        voice: row.get::<_, Option<String>>("voice")?,
                        tts_provider: row.get::<_, Option<String>>("tts_provider")?,
                        command_stt_provider: row
                            .get::<_, Option<String>>("command_stt_provider")?,
                        playback_provider: row.get::<_, Option<String>>("playback_provider")?,
                        command_listening_active: row
                            .get::<_, i64>("command_listening_active")?
                            != 0,
                        command_language: row.get::<_, Option<String>>("command_language")?,
                        audio_reference: row.get::<_, Option<String>>("audio_reference")?,
                        playback_process_id: row
                            .get::<_, Option<i64>>("playback_process_id")?
                            .map(|value| value as u32),
                        runtime_process_id: row
                            .get::<_, Option<i64>>("runtime_process_id")?
                            .map(|value| value as u32),
                        runtime_status: row.get::<_, Option<String>>("runtime_status")?,
                        runtime_error: row.get::<_, Option<String>>("runtime_error")?,
                        startup_cleanup_summary: row
                            .get::<_, Option<String>>("startup_cleanup_summary")?,
                        is_active: row.get::<_, i64>("is_active")? != 0,
                        updated_at: chrono::DateTime::parse_from_rfc3339(
                            &row.get::<_, String>("updated_at")?,
                        )
                        .unwrap()
                        .with_timezone(&chrono::Utc),
                    })
                },
            )
            .optional()
            .unwrap()
    }

    fn deactivate_stale_sessions(&mut self, max_inactive_hours: u32) -> u32 {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let count = connection
            .execute(
                "
                UPDATE sessions
                SET is_active = 0
                WHERE is_active = 1
                  AND updated_at < datetime('now', ? || ' hours')
                ",
                params![format!("-{}", max_inactive_hours)],
            )
            .unwrap();
        count as u32
    }
}

#[derive(Debug, Clone)]
pub struct SQLiteNoteRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteNoteRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl NoteRepository for SQLiteNoteRepository {
    fn ensure_schema(&mut self) {}

    fn save_note(&mut self, note: VoiceNote) {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        connection
            .execute(
                "
                INSERT INTO notes(
                    note_id,
                    session_id,
                    document_id,
                    section_index,
                    chunk_index,
                    char_offset,
                    transcript,
                    transcription_provider,
                    language,
                    raw_audio_path,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(note_id) DO UPDATE SET
                    session_id = excluded.session_id,
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    chunk_index = excluded.chunk_index,
                    char_offset = excluded.char_offset,
                    transcript = excluded.transcript,
                    transcription_provider = excluded.transcription_provider,
                    language = excluded.language,
                    raw_audio_path = excluded.raw_audio_path,
                    created_at = excluded.created_at
                ",
                params![
                    note.note_id,
                    note.session_id,
                    note.document_id,
                    note.position.section_index as i64,
                    note.position.chunk_index as i64,
                    note.position.char_offset as i64,
                    note.transcript,
                    note.transcription_provider,
                    note.language,
                    note.raw_audio_path.map(|path| path.to_string_lossy().to_string()),
                    note.created_at.to_rfc3339(),
                ],
            )
            .unwrap();
    }

    fn list_notes_for_document(&self, document_id: &str) -> Vec<VoiceNote> {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let mut statement = connection
            .prepare("SELECT * FROM notes WHERE document_id = ? ORDER BY created_at ASC")
            .unwrap();
        let rows = statement
            .query_map(params![document_id], note_from_row)
            .unwrap();
        rows.map(|row| row.unwrap()).collect()
    }

    fn search_notes(&self, query: &SearchQuery) -> Vec<SearchResult> {
        let needle = format!("%{}%", query.normalized_text().to_lowercase());
        if needle == "%%" {
            return Vec::new();
        }

        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let mut sql = "
            SELECT note_id, section_index, chunk_index, transcript
            FROM notes
            WHERE LOWER(transcript) LIKE ?
        "
        .to_string();

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(needle)];
        if let Some(document_id) = &query.document_id {
            sql.push_str(" AND document_id = ?");
            params_vec.push(Box::new(document_id.clone()));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT ?");
        params_vec.push(Box::new(query.limit.max(1) as i64));

        let mut statement = connection.prepare(&sql).unwrap();
        let params_ref = params_vec.iter().map(|value| value.as_ref()).collect::<Vec<_>>();
        let rows = statement
            .query_map(rusqlite::params_from_iter(params_ref), |row| {
                Ok(SearchResult {
                    entity_kind: "note".to_string(),
                    entity_id: row.get::<_, String>(0)?,
                    score: 1.0,
                    excerpt: row.get::<_, String>(3)?,
                    anchor: format!(
                        "section:{}/chunk:{}",
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?
                    ),
                })
            })
            .unwrap();

        rows.map(|row| row.unwrap()).collect()
    }
}

#[derive(Debug, Clone)]
pub struct SQLiteRewriteDraftRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteRewriteDraftRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl RewriteDraftRepository for SQLiteRewriteDraftRepository {
    fn ensure_schema(&mut self) {}

    fn save_draft(&mut self, draft: RewriteDraft) {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        connection
            .execute(
                "
                INSERT INTO drafts(
                    draft_id,
                    document_id,
                    section_index,
                    source_anchor,
                    source_excerpt,
                    note_transcripts_json,
                    rewritten_text,
                    provider_name,
                    status,
                    created_at
                )
                VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(draft_id) DO UPDATE SET
                    document_id = excluded.document_id,
                    section_index = excluded.section_index,
                    source_anchor = excluded.source_anchor,
                    source_excerpt = excluded.source_excerpt,
                    note_transcripts_json = excluded.note_transcripts_json,
                    rewritten_text = excluded.rewritten_text,
                    provider_name = excluded.provider_name,
                    status = excluded.status,
                    created_at = excluded.created_at
                ",
                params![
                    draft.draft_id,
                    draft.document_id,
                    draft.section_index as i64,
                    draft.source_anchor,
                    draft.source_excerpt,
                    serde_json::to_string(&draft.note_transcripts).unwrap(),
                    draft.rewritten_text,
                    draft.provider_name,
                    rewrite_status_to_str(draft.status),
                    draft.created_at.to_rfc3339(),
                ],
            )
            .unwrap();
    }

    fn list_drafts_for_document(&self, document_id: &str) -> Vec<RewriteDraft> {
        let connection = self.connection.lock().expect("sqlite connection lock poisoned");
        let mut statement = connection
            .prepare("SELECT * FROM drafts WHERE document_id = ? ORDER BY created_at DESC")
            .unwrap();
        let rows = statement
            .query_map(params![document_id], |row| {
                Ok(RewriteDraft {
                    draft_id: row.get::<_, String>("draft_id")?,
                    document_id: row.get::<_, String>("document_id")?,
                    section_index: row.get::<_, i64>("section_index")? as usize,
                    source_anchor: row.get::<_, String>("source_anchor")?,
                    source_excerpt: row.get::<_, String>("source_excerpt")?,
                    note_transcripts: serde_json::from_str(
                        &row.get::<_, String>("note_transcripts_json")?,
                    )
                    .unwrap_or_default(),
                    rewritten_text: row.get::<_, String>("rewritten_text")?,
                    provider_name: row.get::<_, String>("provider_name")?,
                    status: rewrite_status_from_str(&row.get::<_, String>("status")?),
                    created_at: chrono::DateTime::parse_from_rfc3339(
                        &row.get::<_, String>("created_at")?,
                    )
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                })
            })
            .unwrap();
        rows.map(|row| row.unwrap()).collect()
    }
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<VoiceNote> {
    Ok(VoiceNote {
        note_id: row.get::<_, String>("note_id")?,
        session_id: row.get::<_, String>("session_id")?,
        document_id: row.get::<_, String>("document_id")?,
        position: ReadingPosition {
            section_index: row.get::<_, i64>("section_index")? as usize,
            chunk_index: row.get::<_, i64>("chunk_index")? as usize,
            char_offset: row.get::<_, i64>("char_offset")? as usize,
        },
        transcript: row.get::<_, String>("transcript")?,
        transcription_provider: row.get::<_, String>("transcription_provider")?,
        language: row.get::<_, String>("language")?,
        raw_audio_path: row
            .get::<_, Option<String>>("raw_audio_path")?
            .map(PathBuf::from),
        created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>("created_at")?)
            .unwrap()
            .with_timezone(&chrono::Utc),
    })
}

fn document_to_json(document: &Document) -> String {
    serde_json::json!({
        "document_id": document.document_id,
        "title": document.title,
        "source_path": document.source_path.to_string_lossy(),
        "imported_at": document.imported_at.to_rfc3339(),
        "sections": document.sections.iter().map(|section| {
            serde_json::json!({
                "index": section.index,
                "title": section.title,
                "source_anchor": section.source_anchor,
                "chunks": section.chunks.iter().map(|chunk| {
                    serde_json::json!({
                        "index": chunk.index,
                        "text": chunk.text,
                        "char_start": chunk.char_start,
                        "char_end": chunk.char_end,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    })
    .to_string()
}

fn reader_state_to_str(state: ReaderState) -> &'static str {
    state.as_str()
}

fn reader_state_from_str(value: &str) -> ReaderState {
    match value {
        "idle" => ReaderState::Idle,
        "reading" => ReaderState::Reading,
        "paused" => ReaderState::Paused,
        "listening_for_command" => ReaderState::ListeningForCommand,
        "recording_note" => ReaderState::RecordingNote,
        "processing_rewrite" => ReaderState::ProcessingRewrite,
        "reading_rewrite" => ReaderState::ReadingRewrite,
        "error" => ReaderState::Error,
        _ => ReaderState::Error,
    }
}

fn playback_state_to_str(state: PlaybackState) -> &'static str {
    state.as_str()
}

fn playback_state_from_str(value: &str) -> PlaybackState {
    match value {
        "stopped" => PlaybackState::Stopped,
        "playing" => PlaybackState::Playing,
        "paused" => PlaybackState::Paused,
        _ => PlaybackState::Stopped,
    }
}

fn rewrite_status_to_str(status: RewriteStatus) -> &'static str {
    match status {
        RewriteStatus::Requested => "requested",
        RewriteStatus::Generated => "generated",
        RewriteStatus::Dismissed => "dismissed",
    }
}

fn rewrite_status_from_str(value: &str) -> RewriteStatus {
    match value {
        "requested" => RewriteStatus::Requested,
        "generated" => RewriteStatus::Generated,
        "dismissed" => RewriteStatus::Dismissed,
        _ => RewriteStatus::Requested,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SQLiteDocumentRepository, SQLiteNoteRepository, SQLiteRewriteDraftRepository,
        SQLiteSessionRepository,
    };
    use crate::SQLiteDatabase;
    use marginalia_core::domain::{
        Document, DocumentChunk, DocumentSection, ReadingPosition, ReadingSession, RewriteDraft,
        RewriteStatus, SearchQuery, VoiceNote,
    };
    use marginalia_core::ports::storage::{
        DocumentRepository, NoteRepository, RewriteDraftRepository, SessionRepository,
    };
    use std::path::PathBuf;

    fn test_document() -> Document {
        Document {
            document_id: "doc-1".to_string(),
            title: "Doc".to_string(),
            source_path: PathBuf::from("/tmp/doc.md"),
            sections: vec![DocumentSection {
                index: 0,
                title: "Intro".to_string(),
                chunks: vec![DocumentChunk {
                    index: 0,
                    text: "Alpha beta gamma".to_string(),
                    char_start: 0,
                    char_end: 16,
                }],
                source_anchor: Some("section:0".to_string()),
            }],
            imported_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn sqlite_document_repository_round_trips_document() {
        let database = SQLiteDatabase::open_in_memory().unwrap();
        let mut repository = SQLiteDocumentRepository::new(database.connection());

        repository.save_document(test_document());
        let loaded = repository.get_document("doc-1").unwrap();

        assert_eq!(loaded.title, "Doc");
        assert_eq!(loaded.sections[0].chunks[0].text, "Alpha beta gamma");
    }

    #[test]
    fn sqlite_session_repository_returns_active_session() {
        let database = SQLiteDatabase::open_in_memory().unwrap();
        let mut repository = SQLiteSessionRepository::new(database.connection());
        let session = ReadingSession::new("session-1", "doc-1");

        repository.save_session(session.clone());

        assert_eq!(repository.get_active_session(), Some(session));
    }

    #[test]
    fn sqlite_note_repository_round_trips_note_and_search() {
        let database = SQLiteDatabase::open_in_memory().unwrap();
        let mut repository = SQLiteNoteRepository::new(database.connection());
        repository.save_note(VoiceNote {
            note_id: "note-1".to_string(),
            session_id: "session-1".to_string(),
            document_id: "doc-1".to_string(),
            position: ReadingPosition::default(),
            transcript: "Important passage".to_string(),
            transcription_provider: "fake-dictation".to_string(),
            language: "it".to_string(),
            raw_audio_path: None,
            created_at: chrono::Utc::now(),
        });

        let notes = repository.list_notes_for_document("doc-1");
        let results = repository.search_notes(&SearchQuery {
            text: "passage".to_string(),
            document_id: Some("doc-1".to_string()),
            limit: 10,
        });

        assert_eq!(notes.len(), 1);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn sqlite_rewrite_draft_repository_round_trips_draft() {
        let database = SQLiteDatabase::open_in_memory().unwrap();
        let mut repository = SQLiteRewriteDraftRepository::new(database.connection());
        repository.save_draft(RewriteDraft {
            draft_id: "draft-1".to_string(),
            document_id: "doc-1".to_string(),
            section_index: 0,
            source_anchor: "section:0".to_string(),
            source_excerpt: "Alpha".to_string(),
            note_transcripts: vec!["Note".to_string()],
            rewritten_text: "Rewritten".to_string(),
            provider_name: "fake".to_string(),
            status: RewriteStatus::Generated,
            created_at: chrono::Utc::now(),
        });

        let drafts = repository.list_drafts_for_document("doc-1");
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].status, RewriteStatus::Generated);
    }
}
