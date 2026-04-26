//! Conversions from `marginalia-runtime` / `marginalia-core` value types into
//! the FFI-facing mirror types defined in `src/lib.rs`. Kept in a dedicated
//! module so the main `lib.rs` focuses on the FfiRuntime wiring.

use marginalia_core::domain::VoiceNote;
use marginalia_core::frontend::{
    AppSnapshot as CoreAppSnapshot, DocumentChunkView, DocumentListItem as CoreDocItem,
    DocumentSectionView, DocumentView as CoreDocView, SessionSnapshot as CoreSessionSnapshot,
};
use marginalia_runtime::RuntimeEvent;

use crate::{
    AppSnapshot, ChunkView, DocumentListItem, DocumentView, FfiRuntimeEvent, NoteView,
    PlaybackState, SectionView, SessionSnapshot,
};

// ── Lossy usize → u32 (we never actually see > 2^32 chunks; clamp for safety) ──
#[inline]
fn u32_lossy(v: usize) -> u32 {
    v.min(u32::MAX as usize) as u32
}

#[inline]
fn u32_lossy_opt(v: Option<usize>) -> Option<u32> {
    v.map(u32_lossy)
}

// ── PlaybackState ──────────────────────────────────────────────────────────
impl PlaybackState {
    pub fn from_runtime_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "idle" => PlaybackState::Idle,
            "playing" => PlaybackState::Playing,
            "paused" => PlaybackState::Paused,
            "finished" => PlaybackState::Finished,
            _ => PlaybackState::Unknown,
        }
    }
}

// ── DocumentListItem ───────────────────────────────────────────────────────
impl From<CoreDocItem> for DocumentListItem {
    fn from(d: CoreDocItem) -> Self {
        DocumentListItem {
            id: d.document_id,
            title: d.title,
            chapter_count: u32_lossy(d.chapter_count),
            chunk_count: u32_lossy(d.chunk_count),
        }
    }
}

// ── ChunkView ──────────────────────────────────────────────────────────────
impl From<DocumentChunkView> for ChunkView {
    fn from(c: DocumentChunkView) -> Self {
        ChunkView {
            index: u32_lossy(c.index),
            anchor: c.anchor,
            text: c.text,
            is_active: c.is_active,
            is_read: c.is_read,
        }
    }
}

// ── SectionView ────────────────────────────────────────────────────────────
impl From<DocumentSectionView> for SectionView {
    fn from(s: DocumentSectionView) -> Self {
        SectionView {
            index: u32_lossy(s.index),
            title: s.title,
            source_anchor: s.source_anchor,
            chunks: s.chunks.into_iter().map(Into::into).collect(),
        }
    }
}

// ── DocumentView ───────────────────────────────────────────────────────────
impl From<CoreDocView> for DocumentView {
    fn from(d: CoreDocView) -> Self {
        DocumentView {
            document_id: d.document_id,
            title: d.title,
            source_path: d.source_path,
            chapter_count: u32_lossy(d.chapter_count),
            chunk_count: u32_lossy(d.chunk_count),
            active_section_index: u32_lossy_opt(d.active_section_index),
            active_chunk_index: u32_lossy_opt(d.active_chunk_index),
            sections: d.sections.into_iter().map(Into::into).collect(),
        }
    }
}

// ── SessionSnapshot ────────────────────────────────────────────────────────
impl From<CoreSessionSnapshot> for SessionSnapshot {
    fn from(s: CoreSessionSnapshot) -> Self {
        SessionSnapshot {
            session_id: s.session_id,
            document_id: s.document_id,
            anchor: s.anchor,
            state: s.state,
            playback_state: PlaybackState::from_runtime_string(&s.playback_state),
            section_index: u32_lossy(s.section_index),
            section_count: u32_lossy(s.section_count),
            chunk_index: u32_lossy(s.chunk_index),
            chunk_text: s.chunk_text,
            section_title: s.section_title,
            notes_count: u32_lossy(s.notes_count),
            voice: s.voice,
            tts_provider: s.tts_provider,
            command_stt_provider: s.command_stt_provider,
            command_listening_active: s.command_listening_active,
        }
    }
}

// ── AppSnapshot ────────────────────────────────────────────────────────────
impl From<CoreAppSnapshot> for AppSnapshot {
    fn from(a: CoreAppSnapshot) -> Self {
        AppSnapshot {
            state: a.state,
            document_count: u32_lossy(a.document_count),
            active_session_id: a.active_session_id,
            latest_document_id: a.latest_document_id,
            playback_state: a.playback_state,
            runtime_status: a.runtime_status,
        }
    }
}

// ── NoteView ───────────────────────────────────────────────────────────────
impl From<VoiceNote> for NoteView {
    fn from(n: VoiceNote) -> Self {
        let anchor = n.anchor();
        let section_index = u32_lossy(n.position.section_index);
        let chunk_index = u32_lossy(n.position.chunk_index);
        let audio_reference = n
            .raw_audio_path
            .map(|p| p.display().to_string())
            .filter(|s| !s.is_empty());
        NoteView {
            note_id: n.note_id,
            document_id: n.document_id,
            session_id: n.session_id,
            anchor,
            section_index,
            chunk_index,
            text: n.transcript,
            language: n.language,
            transcription_provider: n.transcription_provider,
            created_at_iso: n.created_at.to_rfc3339(),
            audio_reference,
        }
    }
}

// ── RuntimeEvent → FfiRuntimeEvent ─────────────────────────────────────────
impl From<RuntimeEvent> for FfiRuntimeEvent {
    fn from(e: RuntimeEvent) -> Self {
        match e {
            RuntimeEvent::ChunkAdvanced {
                document_id,
                section_index,
                chunk_index,
            } => FfiRuntimeEvent::ChunkAdvanced {
                document_id,
                section_index: u32_lossy(section_index),
                chunk_index: u32_lossy(chunk_index),
            },
            RuntimeEvent::SynthesisStarted {
                document_id,
                section_index,
                chunk_index,
            } => FfiRuntimeEvent::SynthesisStarted {
                document_id,
                section_index: u32_lossy(section_index),
                chunk_index: u32_lossy(chunk_index),
            },
            RuntimeEvent::SynthesisReady {
                document_id,
                section_index,
                chunk_index,
                cache_hit,
            } => FfiRuntimeEvent::SynthesisReady {
                document_id,
                section_index: u32_lossy(section_index),
                chunk_index: u32_lossy(chunk_index),
                cache_hit,
            },
            RuntimeEvent::PlaybackFinished {
                document_id,
                section_index,
                chunk_index,
            } => FfiRuntimeEvent::PlaybackFinished {
                document_id,
                section_index: u32_lossy(section_index),
                chunk_index: u32_lossy(chunk_index),
            },
            RuntimeEvent::CommandRecognized { raw_text, command } => {
                FfiRuntimeEvent::CommandRecognized { raw_text, command }
            }
            RuntimeEvent::SessionRestored {
                session_id,
                document_id,
                section_index,
                chunk_index,
            } => FfiRuntimeEvent::SessionRestored {
                session_id,
                document_id,
                section_index: u32_lossy(section_index),
                chunk_index: u32_lossy(chunk_index),
            },
            RuntimeEvent::SessionStopped { document_id } => {
                FfiRuntimeEvent::SessionStopped { document_id }
            }
            RuntimeEvent::IngestStarted { source } => FfiRuntimeEvent::IngestStarted { source },
            RuntimeEvent::IngestFinished {
                source,
                document_id,
                error,
            } => FfiRuntimeEvent::IngestFinished {
                source,
                document_id,
                error_message: error,
            },
            RuntimeEvent::DictationStarted => FfiRuntimeEvent::DictationStarted,
            RuntimeEvent::VoiceNoteTranscribed {
                text,
                duration_secs,
                note_id,
                error,
            } => FfiRuntimeEvent::VoiceNoteTranscribed {
                text,
                duration_secs,
                note_id,
                error_message: error,
            },
            RuntimeEvent::VoiceMismatch {
                document_id,
                detected_language,
                current_language,
            } => FfiRuntimeEvent::VoiceMismatch {
                document_id,
                detected_language,
                current_language,
            },
            RuntimeEvent::Error { message } => FfiRuntimeEvent::Error { message },
        }
    }
}
