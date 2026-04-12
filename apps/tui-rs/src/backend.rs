use crate::config::TuiConfig;
use marginalia_playback_host::HostPlaybackEngine;
use marginalia_runtime::{RuntimeFrontend, SqliteRuntime};
#[cfg(feature = "apple-stt")]
use marginalia_stt_apple::new_apple_stt;
#[cfg(feature = "whisper-stt")]
use marginalia_stt_whisper::{
    WhisperCommandRecognizer, WhisperConfig, WhisperDictationTranscriber,
};
use marginalia_tts_kokoro::{
    KokoroConfig, KokoroExternalPhonemizerConfig, KokoroSpeechSynthesizer,
    KokoroSpeechSynthesizerConfig, KokoroTextProcessor,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct BackendLogEntry {
    pub sequence: u64,
    pub line: String,
}

#[derive(Debug, Deserialize)]
pub struct ResponseEnvelope {
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    #[allow(dead_code)]
    pub request_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IngestDocumentResult {
    pub message: String,
    pub document_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppSnapshot {
    pub active_session_id: Option<String>,
    pub document_count: u32,
    pub latest_document_id: Option<String>,
    pub playback_state: Option<String>,
    pub runtime_status: Option<String>,
    pub state: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SessionSnapshot {
    pub anchor: String,
    pub chunk_index: u32,
    pub chunk_text: String,
    pub command_listening_active: bool,
    pub command_stt_provider: Option<String>,
    pub document_id: String,
    pub notes_count: u32,
    pub playback_provider: Option<String>,
    pub playback_state: String,
    pub section_count: u32,
    pub section_index: u32,
    pub section_title: String,
    pub session_id: String,
    pub state: String,
    pub tts_provider: Option<String>,
    pub voice: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentListItem {
    pub chapter_count: u32,
    pub chunk_count: u32,
    pub document_id: String,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentChunkView {
    pub anchor: String,
    pub index: u32,
    pub is_active: bool,
    pub is_read: bool,
    pub text: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentSectionView {
    pub chunks: Vec<DocumentChunkView>,
    pub index: u32,
    pub title: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DocumentView {
    pub active_chunk_index: Option<u32>,
    pub active_section_index: Option<u32>,
    pub chapter_count: u32,
    pub chunk_count: u32,
    pub document_id: String,
    pub sections: Vec<DocumentSectionView>,
    pub source_path: String,
    pub title: String,
}

pub(crate) enum BackendClient {
    Beta(BetaBackendClient),
}

impl BackendClient {
    pub fn spawn() -> Result<Self, String> {
        BetaBackendClient::spawn().map(Self::Beta)
    }

    pub fn mode_label(&self) -> &'static str {
        "Beta Rust runtime"
    }

    pub fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        let Self::Beta(client) = self;
        client.get_app_snapshot()
    }

    pub fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        let Self::Beta(client) = self;
        client.get_session_snapshot()
    }

    pub fn get_doctor_report(&mut self) -> Result<Value, String> {
        let Self::Beta(client) = self;
        client.get_doctor_report()
    }

    pub fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        let Self::Beta(client) = self;
        client.list_documents()
    }

    pub fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        let Self::Beta(client) = self;
        client.get_document_view(document_id)
    }

    pub fn ingest_document(&mut self, path: &Path) -> Result<IngestDocumentResult, String> {
        let response = self.command_response(
            "ingest_document",
            json!({"path": path.display().to_string()}),
        )?;
        let document_id = response
            .payload
            .get("document")
            .and_then(|document| document.get("document_id"))
            .and_then(|document_id| document_id.as_str())
            .map(ToString::to_string);
        Ok(IngestDocumentResult {
            message: response.message,
            document_id,
        })
    }

    /// Dispatch start_session to a background thread. Returns immediately.
    pub fn start_session_async(&mut self, target: &str) {
        self.send_async("start_session", json!({"target": target}));
    }

    /// Dispatch any command to a background thread. Returns immediately.
    /// Used for commands that trigger TTS synthesis to avoid UI freeze.
    /// Silently drops the command if another async command is still running.
    fn send_async(&mut self, name: &str, payload: Value) {
        let Self::Beta(client) = self;
        if client.is_busy() {
            client.push_log(format!("beta command {name} => dropped (busy)"));
            return;
        }
        client.send_command_async(name.to_string(), payload);
    }

    /// Poll for the result of an async command.
    pub fn poll_async_result(&mut self) -> Option<Result<String, String>> {
        let Self::Beta(client) = self;
        let result = client.poll_async_result()?;
        if result.response.status == "ok" {
            Some(Ok(result.response.message))
        } else {
            Some(Err(result.response.message))
        }
    }

    pub fn is_busy(&self) -> bool {
        let Self::Beta(client) = self;
        client.is_busy()
    }

    pub fn pause_session(&mut self) -> Result<String, String> {
        self.command_message("pause_session", json!({}))
    }

    pub fn resume_session(&mut self) {
        self.send_async("resume_session", json!({}));
    }

    pub fn stop_session(&mut self) -> Result<String, String> {
        self.command_message("stop_session", json!({}))
    }

    pub fn repeat_chunk(&mut self) {
        self.send_async("repeat_chunk", json!({}));
    }

    pub fn restart_chapter(&mut self) {
        self.send_async("restart_chapter", json!({}));
    }

    pub fn previous_chunk(&mut self) {
        self.send_async("previous_chunk", json!({}));
    }

    pub fn next_chunk(&mut self) {
        self.send_async("next_chunk", json!({}));
    }

    pub fn previous_chapter(&mut self) {
        self.send_async("previous_chapter", json!({}));
    }

    pub fn next_chapter(&mut self) {
        self.send_async("next_chapter", json!({}));
    }

    pub fn create_note(&mut self, text: &str) -> Result<String, String> {
        self.command_message("create_note", json!({"text": text}))
    }

    fn command_message(&mut self, name: &str, payload: Value) -> Result<String, String> {
        let response = self.command_response(name, payload)?;
        if response.status == "ok" {
            Ok(response.message)
        } else {
            Err(response.message)
        }
    }

    fn command_response(&mut self, name: &str, payload: Value) -> Result<ResponseEnvelope, String> {
        let Self::Beta(client) = self;
        client.execute_command_response(name, payload)
    }

    pub fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        let Self::Beta(client) = self;
        client.recent_stderr_entries(after_sequence)
    }

    /// Returns (raw_text, command). raw_text is for debug logging.
    pub fn poll_voice_event(&mut self) -> Option<(Option<String>, Option<String>)> {
        let Self::Beta(client) = self;
        client.poll_voice_event()
    }

    /// Check if the current chunk finished playing. If so, auto-advance to
    /// the next chunk (or stop at end of document). Returns true if advanced.
    pub fn check_auto_advance(&mut self) -> bool {
        let Self::Beta(client) = self;
        client.check_auto_advance()
    }
}

pub(crate) struct BetaBackendClient {
    runtime: Arc<Mutex<Box<dyn RuntimeFrontend + Send>>>,
    logs: VecDeque<BackendLogEntry>,
    sequence: u64,
    voice_cmd_rx: Option<mpsc::Receiver<(Option<String>, Option<String>)>>,
    /// Receiver for the result of a command running on a background thread.
    async_result_rx: Option<mpsc::Receiver<AsyncCommandResult>>,
}

struct AsyncCommandResult {
    name: String,
    response: ResponseEnvelope,
}

impl BetaBackendClient {
    fn spawn() -> Result<Self, String> {
        let config = TuiConfig::load();

        let db_path = config
            .database_path
            .unwrap_or_else(|| PathBuf::from(".marginalia/beta.sqlite3"));
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut runtime_config = marginalia_runtime::RuntimeConfig::default();
        if let Some(v) = config.chunk_target_chars {
            runtime_config.chunk_target_chars = v;
        }
        let mut runtime = SqliteRuntime::open_with_config(&db_path, runtime_config)
            .map_err(|err| format!("Unable to open beta runtime database: {err}"))?;

        // Playback engine: created here, but NOT passed to the runtime yet.
        // We set the AEC render callback (if needed) after STT init, then hand
        // it off to the runtime.
        let mut playback_engine = if config.playback.fake {
            None
        } else {
            Some(HostPlaybackEngine::default())
        };
        let playback_label = if playback_engine.is_some() { "host" } else { "fake" };

        // TTS: Kokoro se [kokoro] assets_root è configurato
        let mut tts_label = "fake";
        if let Some(assets_root) = config.kokoro.assets_root {
            let kokoro_config = KokoroConfig::from_assets_root(&assets_root);
            let readiness = kokoro_config.readiness_report();
            if readiness.is_ready() {
                let tts_dir = config.kokoro.tts_cache_dir.unwrap_or_else(|| {
                    db_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(".marginalia-tts-cache")
                });
                let synth_config = KokoroSpeechSynthesizerConfig::new(&tts_dir);
                let synthesizer = if let Some(program) = &config.kokoro.phonemizer_program {
                    let args = if config.kokoro.phonemizer_args.is_empty() {
                        vec![
                            "-v".to_string(),
                            "it".to_string(),
                            "--ipa".to_string(),
                            "-q".to_string(),
                        ]
                    } else {
                        config.kokoro.phonemizer_args.clone()
                    };
                    let text_processor = KokoroTextProcessor::with_external_command(
                        kokoro_config.clone(),
                        KokoroExternalPhonemizerConfig {
                            program: program.clone(),
                            args,
                        },
                    );
                    KokoroSpeechSynthesizer::with_text_processor(
                        kokoro_config,
                        synth_config,
                        text_processor,
                    )
                } else {
                    KokoroSpeechSynthesizer::new(kokoro_config, synth_config)
                };
                runtime.set_speech_synthesizer(synthesizer);
                runtime.set_provider_doctor_blob(
                    "kokoro",
                    json!({ "ready": true, "assets_root": assets_root.display().to_string() }),
                );
                tts_label = "kokoro";
            } else {
                runtime.set_provider_doctor_blob(
                    "kokoro",
                    json!({
                        "ready": false,
                        "assets_root": assets_root.display().to_string(),
                        "missing": readiness.missing,
                    }),
                );
            }
        }

        // TTS: MLX (macOS Apple Silicon) — overrides Kokoro ONNX if available
        #[cfg(feature = "mlx-tts")]
        {
            let tts_cache = db_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(".marginalia-tts-cache");
            match marginalia_tts_mlx::MlxSpeechSynthesizer::new(
                &config.mlx.model,
                &config.mlx.voice,
                &tts_cache,
            ) {
                Ok(synth) => {
                    runtime.set_speech_synthesizer(synth);
                    tts_label = "kokoro-mlx";
                }
                Err(e) => {
                    log::warn!("MLX TTS init failed, keeping {tts_label}: {e}");
                }
            }
        }

        // STT engine: one engine handles both commands and dictation, with
        // per-context tuning from [stt.commands] and [stt.dictation].
        #[allow(unused_mut, unused_variables)]
        let mut stt_label = "fake";
        #[allow(unused_mut, unused_variables)]
        let mut dictation_label = "fake";
        let stt_engine = config.stt.engine.to_lowercase();

        if stt_engine == "apple" {
            #[cfg(feature = "apple-stt")]
            {
                let commands = config.voice_commands.all_words();
                // Apple wants BCP-47 (it-IT). Accept ISO ("it") and upgrade.
                let language = match config.stt.language.as_deref() {
                    None => "it-IT".to_string(),
                    Some(l) if l.contains('-') => l.to_string(),
                    Some(l) if l.eq_ignore_ascii_case("it") => "it-IT".to_string(),
                    Some(l) if l.eq_ignore_ascii_case("en") => "en-US".to_string(),
                    Some(l) => l.to_string(),
                };
                let cmd_silence = config.stt.commands.silence_timeout.unwrap_or(0.8);
                let dict_silence = config.stt.dictation.silence_timeout.unwrap_or(1.5);
                let dict_max = config.stt.dictation.max_record_seconds.unwrap_or(60.0);
                match new_apple_stt(&language, commands, cmd_silence, dict_silence, dict_max) {
                    Ok((rec, dict, aec_pipeline)) => {
                        runtime.set_command_recognizer(rec);
                        runtime.set_dictation_transcriber(dict);
                        // Wire the playback engine's reference signal into the
                        // AEC pipeline so echo cancellation has something to subtract.
                        // We extract the render sender (which is Send) instead of
                        // moving the whole pipeline (cpal::Stream is !Send).
                        let render_tx = aec_pipeline.render_sender();
                        if let Some(ref mut pe) = playback_engine {
                            pe.set_play_samples_callback(Box::new(move |samples| {
                                use marginalia_stt_apple::aec_pipeline::RenderCommand;
                                let _ = render_tx.try_send(RenderCommand::SetReference(samples));
                            }));
                        }
                        // Keep the pipeline alive for the entire process. The
                        // AecPipeline holds cpal::Stream (!Send) so we can't
                        // store it in BackendClient (Arc<Mutex>). Leaking is
                        // the standard pattern for process-scoped audio streams.
                        Box::leak(Box::new(aec_pipeline));
                        runtime.set_provider_doctor_blob(
                            "apple_stt",
                            json!({
                                "ready": true,
                                "language": language,
                                "cmd_silence_timeout": cmd_silence,
                                "dict_silence_timeout": dict_silence,
                                "dict_max_seconds": dict_max,
                            }),
                        );
                        stt_label = "apple";
                        dictation_label = "apple";
                    }
                    Err(e) => {
                        runtime.set_provider_doctor_blob(
                            "apple_stt",
                            json!({ "ready": false, "error": e }),
                        );
                        log::error!("[apple-stt] {e}");
                    }
                }
            }
            #[cfg(not(feature = "apple-stt"))]
            log::warn!("[stt] engine=apple but apple-stt feature is not built in");
        } else if stt_engine == "whisper" {
            #[cfg(feature = "whisper-stt")]
            if let Some(model_path) = config.stt.whisper.model_path.clone() {
                // Whisper wants ISO ("it"). Accept BCP-47 ("it-IT") and downgrade.
                let language = config
                    .stt
                    .language
                    .clone()
                    .map(|l| l.split('-').next().unwrap_or("it").to_string())
                    .unwrap_or_else(|| "it".to_string());

                // Command-context Whisper: short defaults tuned for trigger words.
                let mut whisper_command_config = WhisperConfig::new(&model_path);
                whisper_command_config.language = language.clone();
                whisper_command_config.max_duration_seconds = 4.0;
                whisper_command_config.silence_timeout_seconds = 0.8;
                if let Some(v) = config.stt.commands.silence_timeout {
                    whisper_command_config.silence_timeout_seconds = v;
                }
                if let Some(v) = config.stt.commands.max_record_seconds {
                    whisper_command_config.max_duration_seconds = v;
                }
                if let Some(v) = config.stt.commands.speech_threshold {
                    whisper_command_config.speech_threshold = v;
                }
                let cmd_commands = config.voice_commands.all_words();
                let whisper_cmd_rec = WhisperCommandRecognizer::new(
                    whisper_command_config,
                    cmd_commands,
                );

                // Dictation-context Whisper: long defaults from WhisperConfig::new
                // (60s, 1.5s, 500), then [stt.dictation] overrides.
                let mut whisper_dictation_config = WhisperConfig::new(&model_path);
                whisper_dictation_config.language = language;
                if let Some(v) = config.stt.dictation.silence_timeout {
                    whisper_dictation_config.silence_timeout_seconds = v;
                }
                if let Some(v) = config.stt.dictation.max_record_seconds {
                    whisper_dictation_config.max_duration_seconds = v;
                }
                if let Some(v) = config.stt.dictation.speech_threshold {
                    whisper_dictation_config.speech_threshold = v;
                }
                let whisper_dict = WhisperDictationTranscriber::new(
                    whisper_dictation_config.clone(),
                );
                runtime.set_stt_engine(marginalia_runtime::SttEngineOutput {
                    command_recognizer: Box::new(whisper_cmd_rec),
                    dictation_transcriber: Box::new(whisper_dict),
                    engine_label: "whisper".to_string(),
                });
                stt_label = "whisper";
                runtime.set_provider_doctor_blob(
                    "whisper_dictation_stt",
                    json!({
                        "ready": true,
                        "model_path": model_path.display().to_string(),
                        "max_record_seconds": whisper_dictation_config.max_duration_seconds,
                        "silence_timeout": whisper_dictation_config.silence_timeout_seconds,
                    }),
                );
                dictation_label = "whisper";
            }
            #[cfg(not(feature = "whisper-stt"))]
            log::warn!("[stt] engine=whisper but whisper-stt feature is not built in");
        } else {
            log::error!(
                "[stt] unknown engine '{stt_engine}' — valid choices: \"apple\", \"whisper\""
            );
        }

        // Now hand the playback engine to the runtime (after AEC callback is wired).
        if let Some(pe) = playback_engine {
            runtime.set_playback_engine(pe);
        }

        // Voice command monitor — open and run in background thread.
        // The monitor is independent from the runtime after creation (owns its own audio stream).
        // Thread exits automatically when voice_cmd_rx is dropped (tx.send fails).
        let stt_debug = config.stt.debug;
        let voice_cmd_rx = {
            let mut monitor = runtime.open_command_monitor();
            // Channel sends (raw_text, command). raw_text is for debug logging.
            let (tx, rx) = std::sync::mpsc::channel::<(Option<String>, Option<String>)>();
            std::thread::spawn(move || {
                loop {
                    let capture = monitor.capture_next_interrupt(Some(2.0));
                    // Log errors from the monitor (e.g. audio stream failures)
                    if let Some(raw) = &capture.raw_text {
                        if raw.starts_with("error:") {
                            log::warn!("[voice-monitor] {raw}");
                            std::thread::sleep(std::time::Duration::from_secs(5));
                            continue;
                        }
                    }

                    let raw = capture.raw_text.filter(|t| !t.is_empty());
                    let cmd = capture.recognized_command;

                    if stt_debug && raw.is_some() || cmd.is_some() {
                        if tx.send((raw, cmd)).is_err() {
                            break;
                        }
                    }
                }
            });
            Some(rx)
        };

        let mut client = Self {
            runtime: Arc::new(Mutex::new(
                Box::new(runtime) as Box<dyn RuntimeFrontend + Send>
            )),
            logs: VecDeque::with_capacity(256),
            sequence: 0,
            voice_cmd_rx,
            async_result_rx: None,
        };
        client.push_log(format!(
            "beta-runtime ready db={} playback={} tts={} stt={} dictation={}",
            db_path.display(),
            playback_label,
            tts_label,
            stt_label,
            dictation_label,
        ));

        // Try to restore the last active session so the user picks up where
        // they left off. The session resumes in Paused state — /resume or
        // "riprendi" starts playback.
        let restore = client.send_request("command", "restore_session", json!({}));
        if restore.status == "ok" {
            if let Some(session) = restore.payload.get("session") {
                if !session.is_null() {
                    let doc = session
                        .get("document_id")
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    let sec = session.get("section_index").and_then(Value::as_u64).unwrap_or(0);
                    let chk = session.get("chunk_index").and_then(Value::as_u64).unwrap_or(0);
                    client.push_log(format!(
                        "Restored session: document={doc} section={sec} chunk={chk}. Type /resume to continue."
                    ));
                }
            }
        }

        Ok(client)
    }

    pub fn poll_voice_event(&mut self) -> Option<(Option<String>, Option<String>)> {
        self.voice_cmd_rx.as_ref()?.try_recv().ok()
    }

    pub fn check_auto_advance(&mut self) -> bool {
        // Use "query" (try_lock) instead of "command" (blocking lock) so we
        // never stall the UI waiting for a synthesis to finish.
        let response = self.send_request("query", "auto_advance", json!({}));
        if response.status == "skipped" {
            return false;
        }
        response
            .payload
            .get("advanced")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn get_app_snapshot(&mut self) -> Result<AppSnapshot, String> {
        let response = self.send_request("query", "get_app_snapshot", json!({}));
        if response.status == "skipped" {
            return Err("skipped".to_string());
        }
        decode_payload(response.payload, "app")
    }

    fn get_session_snapshot(&mut self) -> Result<Option<SessionSnapshot>, String> {
        let response = self.send_request("query", "get_session_snapshot", json!({}));
        if response.status == "skipped" {
            return Err("skipped".to_string());
        }
        match response.payload.get("session") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "session").map(Some),
        }
    }

    fn get_doctor_report(&mut self) -> Result<Value, String> {
        let response = self.send_request("query", "get_doctor_report", json!({}));
        if response.status == "skipped" {
            return Err("skipped".to_string());
        }
        Ok(response.payload)
    }

    fn list_documents(&mut self) -> Result<Vec<DocumentListItem>, String> {
        let response = self.send_request("query", "list_documents", json!({}));
        if response.status == "skipped" {
            return Err("skipped".to_string());
        }
        let documents = response
            .payload
            .get("documents")
            .cloned()
            .ok_or_else(|| "Beta runtime omitted documents list.".to_string())?;
        serde_json::from_value(documents)
            .map_err(|err| format!("Unable to decode documents payload: {err}"))
    }

    fn get_document_view(
        &mut self,
        document_id: Option<&str>,
    ) -> Result<Option<DocumentView>, String> {
        let payload = match document_id {
            Some(document_id) => json!({ "document_id": document_id }),
            None => json!({}),
        };
        let response = self.send_request("query", "get_document_view", payload);
        if response.status == "skipped" {
            return Err("skipped".to_string());
        }
        match response.payload.get("document") {
            Some(Value::Null) | None => Ok(None),
            Some(_) => decode_payload(response.payload, "document").map(Some),
        }
    }

    fn execute_command_response(
        &mut self,
        name: &str,
        payload: Value,
    ) -> Result<ResponseEnvelope, String> {
        Ok(self.send_request("command", name, payload))
    }

    fn recent_stderr_entries(&self, after_sequence: u64) -> Vec<BackendLogEntry> {
        self.logs
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect()
    }

    fn push_log(&mut self, line: String) {
        self.sequence += 1;
        if self.logs.len() >= 200 {
            self.logs.pop_front();
        }
        self.logs.push_back(BackendLogEntry {
            sequence: self.sequence,
            line,
        });
    }
    fn send_request(&mut self, request_type: &str, name: &str, payload: Value) -> ResponseEnvelope {
        // For queries: use try_lock to avoid blocking the UI while prefetch
        // holds the runtime lock. If busy, return a skip response.
        let mut runtime = if request_type == "query" {
            match self.runtime.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    // Runtime busy (prefetch running) — skip this poll cycle
                    return ResponseEnvelope {
                        status: "skipped".to_string(),
                        message: String::new(),
                        payload: json!({}),
                        request_id: None,
                    };
                }
            }
        } else {
            self.runtime.lock().expect("runtime lock poisoned")
        };

        let response = match request_type {
            "query" => runtime.execute_frontend_query(name, payload),
            "command" => runtime.execute_frontend_command(name, payload),
            other => marginalia_runtime::RuntimeFrontendResponse {
                status: "error".to_string(),
                message: format!("Unsupported beta request type: {other}"),
                payload: json!({}),
            },
        };
        drop(runtime);

        // Log commands and query errors — skip routine polling (queries + auto_advance).
        let is_silent = name == "auto_advance";
        if (request_type == "command" && !is_silent) || response.status != "ok" {
            self.push_log(format!("beta {request_type} {name} => {}", response.status));
        }
        ResponseEnvelope {
            status: response.status,
            message: response.message,
            payload: response.payload,
            request_id: None,
        }
    }

    /// Dispatch a command to a background thread. Returns immediately.
    fn send_command_async(&mut self, name: String, payload: Value) {
        let runtime = Arc::clone(&self.runtime);
        let (tx, rx) = mpsc::channel();
        let cmd_name = name.clone();
        std::thread::spawn(move || {
            let mut rt = runtime.lock().expect("runtime lock poisoned");
            let response = rt.execute_frontend_command(&name, payload);
            let _ = tx.send(AsyncCommandResult {
                name,
                response: ResponseEnvelope {
                    status: response.status,
                    message: response.message,
                    payload: response.payload,
                    request_id: None,
                },
            });
        });
        self.push_log(format!("beta command {cmd_name} => dispatched (async)"));
        self.async_result_rx = Some(rx);
    }

    /// Spawn a fire-and-forget prefetch thread for the next chunk.
    /// Waits 300ms before locking so the UI can refresh the document view first.
    fn spawn_prefetch(&mut self) {
        let runtime = Arc::clone(&self.runtime);
        std::thread::spawn(move || {
            // Let the UI refresh at least one cycle before we grab the lock
            std::thread::sleep(std::time::Duration::from_millis(300));
            let mut rt = match runtime.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    // Runtime still locked — skip
                    return;
                }
            };
            rt.execute_frontend_command("prefetch_next", json!({}));
        });
    }

    /// Commands that should trigger a prefetch after completion.
    fn is_navigation_command(name: &str) -> bool {
        matches!(
            name,
            "start_session"
                | "next_chunk"
                | "previous_chunk"
                | "next_chapter"
                | "previous_chapter"
                | "restart_chapter"
                | "repeat_chunk"
                | "resume_session"
        )
    }

    /// Poll for the result of an async command. Returns `None` if still pending.
    fn poll_async_result(&mut self) -> Option<AsyncCommandResult> {
        let rx = self.async_result_rx.as_ref()?;
        match rx.try_recv() {
            Ok(result) => {
                self.push_log(format!(
                    "beta command {} => {} (async complete)",
                    result.name, result.response.status
                ));

                // On successful navigation, prefetch next chunk in background
                if result.response.status == "ok" && Self::is_navigation_command(&result.name) {
                    self.spawn_prefetch();
                }

                self.async_result_rx = None;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.async_result_rx = None;
                None
            }
        }
    }

    fn is_busy(&self) -> bool {
        self.async_result_rx.is_some()
    }
}

fn decode_payload<T>(payload: Value, field: &str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let value = payload
        .get(field)
        .cloned()
        .ok_or_else(|| format!("Backend omitted '{field}' payload."))?;
    serde_json::from_value(value)
        .map_err(|err| format!("Unable to decode '{field}' payload: {err}"))
}
