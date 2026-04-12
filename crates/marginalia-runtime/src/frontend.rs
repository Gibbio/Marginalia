use crate::SqliteRuntime;
use marginalia_core::domain::SearchQuery;
use marginalia_core::frontend::BackendCapabilities;
use marginalia_core::ports::storage::{DocumentRepository, NoteRepository, SessionRepository};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeFrontendResponse {
    pub status: String,
    pub message: String,
    pub payload: Value,
}

/// Stable interface that apps use to talk to the runtime.
/// Apps must never depend on a concrete runtime type — only on this trait.
pub trait RuntimeFrontend {
    fn execute_frontend_query(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse;
    fn execute_frontend_command(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse;
}

impl RuntimeFrontend for SqliteRuntime {
    fn execute_frontend_query(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse {
        self.frontend_query(name, payload)
    }

    fn execute_frontend_command(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse {
        self.frontend_command(name, payload)
    }
}

impl SqliteRuntime {
    pub fn backend_capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            protocol_version: 1,
            commands: vec![
                "create_note",
                "ingest_document",
                "next_chunk",
                "next_chapter",
                "pause_session",
                "previous_chapter",
                "previous_chunk",
                "repeat_chunk",
                "restart_chapter",
                "restore_session",
                "resume_session",
                "start_session",
                "stop_session",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            queries: vec![
                "get_app_snapshot",
                "get_backend_capabilities",
                "get_document_view",
                "get_doctor_report",
                "get_session_snapshot",
                "list_documents",
                "list_notes",
                "search_documents",
                "search_notes",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            transports: vec!["embedded".to_string()],
            frontend_event_stream_supported: false,
            dictation_enabled: false,
            rewrite_enabled: false,
            summary_enabled: false,
        }
    }

    fn frontend_query(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse {
        match name {
            "get_app_snapshot" => ok_response(
                "App snapshot loaded.",
                json!({ "app": app_snapshot_to_json(self.app_snapshot()) }),
            ),
            "get_session_snapshot" => match self.session_snapshot() {
                Ok(snapshot) => ok_response(
                    "Session snapshot loaded.",
                    json!({
                        "session": snapshot.map(session_snapshot_to_json).unwrap_or(Value::Null)
                    }),
                ),
                Err(err) => error_response(format!("Unable to load session snapshot: {err}")),
            },
            "get_document_view" => {
                let document_id = payload.get("document_id").and_then(Value::as_str);
                ok_response(
                    "Document view loaded.",
                    json!({
                        "document": self
                            .document_view(document_id)
                            .map(document_view_to_json)
                            .unwrap_or(Value::Null)
                    }),
                )
            }
            "get_doctor_report" => ok_response("Doctor report loaded.", self.doctor_report()),
            "get_backend_capabilities" => ok_response(
                "Backend capabilities loaded.",
                json!({ "capabilities": backend_capabilities_to_json(self.backend_capabilities()) }),
            ),
            "list_documents" => ok_response(
                "Documents listed.",
                json!({
                    "documents": self
                        .list_documents()
                        .into_iter()
                        .map(document_list_item_to_json)
                        .collect::<Vec<_>>()
                }),
            ),
            "list_notes" => {
                let document_id = resolve_document_id(&self.session_repository, &payload);
                match document_id {
                    Some(document_id) => ok_response(
                        "Notes listed.",
                        json!({
                            "notes": self
                                .note_repository
                                .list_notes_for_document(&document_id)
                                .into_iter()
                                .map(note_view_to_json)
                                .collect::<Vec<_>>(),
                            "document_id": document_id,
                        }),
                    ),
                    None => ok_response(
                        "No document selected for notes.",
                        json!({ "notes": [], "document_id": Value::Null }),
                    ),
                }
            }
            "search_documents" => {
                let query = parse_search_query(&payload);
                if query.normalized_text().is_empty() {
                    return error_response("search_documents requires a query.".to_string());
                }
                ok_response(
                    "Document search complete.",
                    json!({
                        "search": {
                            "query": query.text,
                            "results": self
                                .document_repository
                                .search_documents(&query)
                                .into_iter()
                                .map(search_result_to_json)
                                .collect::<Vec<_>>(),
                        }
                    }),
                )
            }
            "search_notes" => {
                let query = parse_search_query(&payload);
                if query.normalized_text().is_empty() {
                    return error_response("search_notes requires a query.".to_string());
                }
                ok_response(
                    "Note search complete.",
                    json!({
                        "search": {
                            "query": query.text,
                            "results": self
                                .note_repository
                                .search_notes(&query)
                                .into_iter()
                                .map(search_result_to_json)
                                .collect::<Vec<_>>(),
                        }
                    }),
                )
            }
            other => error_response(format!("Unsupported beta query: {other}")),
        }
    }

    fn frontend_command(&mut self, name: &str, payload: Value) -> RuntimeFrontendResponse {
        match name {
            "ingest_document" => {
                let Some(path) = payload.get("path").and_then(Value::as_str) else {
                    return error_response("ingest_document requires a path.".to_string());
                };
                match self.ingest_path(Path::new(path)) {
                    Ok(outcome) => ok_response(
                        "Document ingested into local SQLite storage.",
                        json!({
                            "document": {
                                "document_id": outcome.document.document_id,
                                "title": outcome.document.title,
                            }
                        }),
                    ),
                    Err(err) => error_response(err.to_string()),
                }
            }
            "auto_advance" => {
                let advanced = self.try_auto_advance();
                ok_response(
                    if advanced {
                        "Advanced to next chunk."
                    } else {
                        "No advance needed."
                    },
                    json!({ "advanced": advanced }),
                )
            }
            "restore_session" => match self.restore_session() {
                Some(session) => ok_response(
                    "Session restored.",
                    json!({
                        "session": {
                            "session_id": session.session_id,
                            "document_id": session.document_id,
                            "section_index": session.position.section_index,
                            "chunk_index": session.position.chunk_index,
                        }
                    }),
                ),
                None => ok_response("No active session to restore.", json!(null)),
            },
            "start_session" => {
                let target = payload
                    .get("target")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if target.is_empty() {
                    return error_response("start_session requires a target.".to_string());
                }

                let document_id = if Path::new(&target).exists() {
                    match self.ingest_path(Path::new(&target)) {
                        Ok(outcome) => outcome.document.document_id,
                        Err(err) => return error_response(err.to_string()),
                    }
                } else {
                    target
                };

                match self.start_session(&document_id) {
                    Ok(session) => ok_response(
                        "Reading session started.",
                        json!({
                            "session": {
                                "session_id": session.session_id,
                                "document_id": session.document_id,
                            }
                        }),
                    ),
                    Err(err) => error_response(err.to_string()),
                }
            }
            "pause_session" => {
                simple_command_response(self.pause_session(), "Reading session paused.")
            }
            "resume_session" => {
                simple_command_response(self.resume_session(), "Reading session resumed.")
            }
            "stop_session" => {
                simple_command_response(self.stop_session(), "Reading session stopped.")
            }
            "repeat_chunk" => {
                simple_command_response(self.repeat_chunk(), "Current chunk repeated.")
            }
            "restart_chapter" => {
                simple_command_response(self.restart_chapter(), "Current chapter restarted.")
            }
            "previous_chunk" => {
                simple_command_response(self.previous_chunk(), "Moved to previous chunk.")
            }
            "next_chunk" => simple_command_response(self.next_chunk(), "Moved to next chunk."),
            "previous_chapter" => {
                simple_command_response(self.previous_chapter(), "Moved to previous chapter.")
            }
            "next_chapter" => {
                simple_command_response(self.next_chapter(), "Moved to next chapter.")
            }
            "prefetch_next" => {
                self.prefetch_next();
                ok_response("Prefetch done.", json!({}))
            }
            "create_note" => {
                let text = payload
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match self.create_note(text) {
                    Ok(note) => {
                        ok_response("Note saved.", json!({ "note": note_view_to_json(note) }))
                    }
                    Err(err) => error_response(err.to_string()),
                }
            }
            other => error_response(format!("Unsupported beta command: {other}")),
        }
    }
}

fn simple_command_response(
    result: Result<(), impl ToString>,
    message: impl Into<String>,
) -> RuntimeFrontendResponse {
    match result {
        Ok(()) => ok_response(message.into(), json!({})),
        Err(err) => error_response(err.to_string()),
    }
}

fn resolve_document_id<S>(session_repository: &S, payload: &Value) -> Option<String>
where
    S: SessionRepository,
{
    payload
        .get("document_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            session_repository
                .get_active_session()
                .map(|session| session.document_id)
        })
}

fn parse_search_query(payload: &Value) -> SearchQuery {
    SearchQuery {
        text: payload
            .get("query")
            .or_else(|| payload.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        document_id: payload
            .get("document_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        limit: payload
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(20),
    }
}

fn ok_response(message: impl Into<String>, payload: Value) -> RuntimeFrontendResponse {
    RuntimeFrontendResponse {
        status: "ok".to_string(),
        message: message.into(),
        payload,
    }
}

fn error_response(message: impl Into<String>) -> RuntimeFrontendResponse {
    RuntimeFrontendResponse {
        status: "error".to_string(),
        message: message.into(),
        payload: json!({}),
    }
}

fn backend_capabilities_to_json(capabilities: BackendCapabilities) -> Value {
    json!({
        "protocol_version": capabilities.protocol_version,
        "commands": capabilities.commands,
        "queries": capabilities.queries,
        "transports": capabilities.transports,
        "frontend_event_stream_supported": capabilities.frontend_event_stream_supported,
        "dictation_enabled": capabilities.dictation_enabled,
        "rewrite_enabled": capabilities.rewrite_enabled,
        "summary_enabled": capabilities.summary_enabled,
    })
}

fn app_snapshot_to_json(snapshot: marginalia_core::frontend::AppSnapshot) -> Value {
    json!({
        "active_session_id": snapshot.active_session_id,
        "document_count": snapshot.document_count,
        "latest_document_id": snapshot.latest_document_id,
        "playback_state": snapshot.playback_state,
        "runtime_status": snapshot.runtime_status,
        "state": snapshot.state,
    })
}

fn session_snapshot_to_json(snapshot: marginalia_core::frontend::SessionSnapshot) -> Value {
    json!({
        "anchor": snapshot.anchor,
        "chunk_index": snapshot.chunk_index,
        "chunk_text": snapshot.chunk_text,
        "command_listening_active": snapshot.command_listening_active,
        "command_stt_provider": snapshot.command_stt_provider,
        "document_id": snapshot.document_id,
        "notes_count": snapshot.notes_count,
        "playback_provider": snapshot.playback_provider,
        "playback_state": snapshot.playback_state,
        "section_count": snapshot.section_count,
        "section_index": snapshot.section_index,
        "section_title": snapshot.section_title,
        "session_id": snapshot.session_id,
        "state": snapshot.state,
        "tts_provider": snapshot.tts_provider,
        "voice": snapshot.voice,
    })
}

fn document_list_item_to_json(item: marginalia_core::frontend::DocumentListItem) -> Value {
    json!({
        "chapter_count": item.chapter_count,
        "chunk_count": item.chunk_count,
        "document_id": item.document_id,
        "title": item.title,
    })
}

fn document_view_to_json(view: marginalia_core::frontend::DocumentView) -> Value {
    json!({
        "active_chunk_index": view.active_chunk_index,
        "active_section_index": view.active_section_index,
        "chapter_count": view.chapter_count,
        "chunk_count": view.chunk_count,
        "document_id": view.document_id,
        "sections": view
            .sections
            .into_iter()
            .map(document_section_view_to_json)
            .collect::<Vec<_>>(),
        "source_path": view.source_path,
        "title": view.title,
    })
}

fn document_section_view_to_json(section: marginalia_core::frontend::DocumentSectionView) -> Value {
    json!({
        "chunk_count": section.chunk_count,
        "chunks": section
            .chunks
            .into_iter()
            .map(document_chunk_view_to_json)
            .collect::<Vec<_>>(),
        "index": section.index,
        "source_anchor": section.source_anchor,
        "title": section.title,
    })
}

fn document_chunk_view_to_json(chunk: marginalia_core::frontend::DocumentChunkView) -> Value {
    json!({
        "anchor": chunk.anchor,
        "char_end": chunk.char_end,
        "char_start": chunk.char_start,
        "index": chunk.index,
        "is_active": chunk.is_active,
        "is_read": chunk.is_read,
        "text": chunk.text,
    })
}

fn note_view_to_json(note: marginalia_core::domain::VoiceNote) -> Value {
    json!({
        "anchor": note.anchor(),
        "created_at": note.created_at.to_rfc3339(),
        "document_id": note.document_id,
        "language": note.language,
        "note_id": note.note_id,
        "section_index": note.position.section_index,
        "chunk_index": note.position.chunk_index,
        "session_id": note.session_id,
        "transcript": note.transcript,
        "transcription_provider": note.transcription_provider,
    })
}

fn search_result_to_json(result: marginalia_core::domain::SearchResult) -> Value {
    json!({
        "anchor": result.anchor,
        "entity_id": result.entity_id,
        "entity_kind": result.entity_kind,
        "excerpt": result.excerpt,
        "score": result.score,
    })
}

#[cfg(test)]
mod tests {
    use crate::{RuntimeFrontend, SqliteRuntime};
    use serde_json::json;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(extension: &str) -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "marginalia-runtime-frontend-test-{}.{}",
            timestamp, extension
        ))
    }

    #[test]
    fn frontend_gateway_supports_ingest_and_snapshot_queries() {
        let path = temp_path("md");
        fs::write(&path, "# Intro\n\nAlpha beta gamma.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let ingest = runtime.execute_frontend_command(
            "ingest_document",
            json!({ "path": path.display().to_string() }),
        );
        let app = runtime.execute_frontend_query("get_app_snapshot", json!({}));

        assert_eq!(ingest.status, "ok");
        assert_eq!(app.status, "ok");
        assert_eq!(app.payload["app"]["document_count"], 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn frontend_gateway_supports_start_session_and_document_view() {
        let path = temp_path("txt");
        fs::write(&path, "Alpha beta gamma delta epsilon zeta eta theta.").unwrap();

        let mut runtime = SqliteRuntime::open_in_memory().unwrap();
        let ingest = runtime.execute_frontend_command(
            "ingest_document",
            json!({ "path": path.display().to_string() }),
        );
        let document_id = ingest.payload["document"]["document_id"]
            .as_str()
            .unwrap()
            .to_string();
        let start =
            runtime.execute_frontend_command("start_session", json!({ "target": document_id }));
        let view = runtime.execute_frontend_query("get_document_view", json!({}));

        assert_eq!(start.status, "ok");
        assert_eq!(view.status, "ok");
        assert!(view.payload["document"]["sections"].is_array());

        let _ = fs::remove_file(path);
    }
}
