//! Runtime-side provider reconfiguration: swap TTS voice, TTS backend,
//! STT engine, or language on a live `SqliteRuntime` without restarting the
//! process.
//!
//! The intended caller is the GUI's "Apply" button in the Settings page.
//! The GUI collects the user's choices, builds a [`ProviderSpec`], and calls
//! [`apply_provider_spec`]. The function rebuilds only the providers that
//! changed and returns an [`ApplyReport`] describing what was swapped.
//!
//! # Architecture notes
//!
//! - The Apple STT helper is a persistent subprocess: rebuilding it means
//!   dropping the old `Arc<AppleHelperShared>` (which `kill()`s the child in
//!   its `Drop` impl) and spawning a fresh one with the new locale / command
//!   list. Typical respawn cost is ~0.3–0.8 s; the GUI should show a spinner.
//! - The AEC pipeline owns a `cpal::Stream` (`!Send`). Rebuilding it requires
//!   dropping the old `AecPipeline` first — the stream closes, the cpal
//!   thread exits, and the AEC worker thread observes the disconnected mic
//!   channel and terminates. Then a new pipeline is started.
//! - The playback callback that feeds the AEC render reference is wired to a
//!   stable [`AecRenderSlot`]. The slot's sender is replaced on respawn; the
//!   callback itself is never reinstalled.

use crate::builder::RuntimeSidecar;
use crate::SqliteRuntime;
use marginalia_config::{MlxSection, SttSection, VoiceCommandsSection};
use std::path::Path;
#[cfg(all(feature = "apple-stt", feature = "host-playback"))]
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The user's current selection of providers. Equality-checked field by field
/// against the live configuration to decide what must be rebuilt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSpec {
    /// `"mlx"` | `"kokoro"` | `"fake"`. Determines which TTS backend is used.
    pub tts_backend: String,
    /// Voice id (e.g. `"if_sara"`). Interpreted by the TTS backend.
    pub voice: String,
    /// `"apple"` | `"whisper"` | `"fake"`.
    pub stt_engine: String,
    /// BCP-47 locale (`"it-IT"`, `"en-US"`, …).
    pub language: String,
}

/// What [`apply_provider_spec`] actually did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub tts_swapped: bool,
    pub stt_swapped: bool,
    pub language_changed: bool,
    pub elapsed_ms: u64,
}

/// Shared slot for the AEC render sender. Kept stable across STT respawns
/// so the playback engine's `play_samples_callback` never needs to be
/// re-installed.
#[cfg(all(feature = "apple-stt", feature = "host-playback"))]
#[derive(Clone, Default)]
pub struct AecRenderSlot {
    inner: Arc<
        Mutex<
            Option<
                std::sync::mpsc::SyncSender<marginalia_stt_apple::aec_pipeline::RenderCommand>,
            >,
        >,
    >,
}

#[cfg(all(feature = "apple-stt", feature = "host-playback"))]
impl AecRenderSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the sender (called when the AEC pipeline is (re)built).
    /// Passing a new sender drops the old one; the previous AEC worker thread
    /// will see the disconnect on its next loop iteration.
    pub fn install(
        &self,
        tx: std::sync::mpsc::SyncSender<marginalia_stt_apple::aec_pipeline::RenderCommand>,
    ) {
        *self.inner.lock().unwrap() = Some(tx);
    }

    /// Forward a new render reference to the current AEC pipeline, if any.
    /// Called from the playback engine's `play_samples_callback` for every
    /// chunk that starts playing.
    pub fn send_set_reference(&self, samples: Vec<f32>) {
        use marginalia_stt_apple::aec_pipeline::RenderCommand;
        if let Some(tx) = self.inner.lock().unwrap().as_ref() {
            let _ = tx.try_send(RenderCommand::SetReference(samples));
        }
    }

    /// Clear the current render reference. Called when playback stops.
    pub fn send_clear(&self) {
        use marginalia_stt_apple::aec_pipeline::RenderCommand;
        if let Some(tx) = self.inner.lock().unwrap().as_ref() {
            let _ = tx.try_send(RenderCommand::ClearReference);
        }
    }

    /// Freeze the AEC reference (used on `pause()`). The reference buffer
    /// stays in memory; only `render_pos` advancement and the TTS meter
    /// peak are gated off until `send_resume()` flips them back on.
    pub fn send_pause(&self) {
        use marginalia_stt_apple::aec_pipeline::RenderCommand;
        if let Some(tx) = self.inner.lock().unwrap().as_ref() {
            let _ = tx.try_send(RenderCommand::PauseReference);
        }
    }

    /// Resume advancing through the AEC reference (used on `resume()`).
    pub fn send_resume(&self) {
        use marginalia_stt_apple::aec_pipeline::RenderCommand;
        if let Some(tx) = self.inner.lock().unwrap().as_ref() {
            let _ = tx.try_send(RenderCommand::ResumeReference);
        }
    }
}

/// Normalize a language string to the BCP-47 form SFSpeechRecognizer expects.
/// Extracted from the builder so both `build()` and `apply_provider_spec`
/// agree on the mapping.
pub fn normalize_apple_language(lang: &Option<String>) -> String {
    match lang.as_deref() {
        None => "it-IT".to_string(),
        Some(l) if l.contains('-') => l.to_string(),
        Some(l) if l.eq_ignore_ascii_case("it") => "it-IT".to_string(),
        Some(l) if l.eq_ignore_ascii_case("en") => "en-US".to_string(),
        Some(l) if l.eq_ignore_ascii_case("fr") => "fr-FR".to_string(),
        Some(l) if l.eq_ignore_ascii_case("de") => "de-DE".to_string(),
        Some(l) if l.eq_ignore_ascii_case("es") => "es-ES".to_string(),
        Some(l) if l.eq_ignore_ascii_case("pt") => "pt-BR".to_string(),
        Some(l) if l.eq_ignore_ascii_case("ja") => "ja-JP".to_string(),
        Some(l) if l.eq_ignore_ascii_case("zh") => "zh-CN".to_string(),
        Some(l) => l.to_string(),
    }
}

/// Convert BCP-47 to the ISO-639-1 form Whisper expects (`"it-IT"` → `"it"`).
pub fn normalize_whisper_language(lang: &str) -> String {
    lang.split('-').next().unwrap_or(lang).to_string()
}

// ──────────────────────────────────────────────────────────────────────
// The reconfigure entry point
// ──────────────────────────────────────────────────────────────────────

/// Context held by the caller (tui-rs, the future GUI) across the lifetime
/// of a `SqliteRuntime`. Carries the live configuration for each provider
/// so `apply_provider_spec` can diff against it and decide what to rebuild.
pub struct ReconfigureContext {
    pub mlx: MlxSection,
    pub stt: SttSection,
    pub voice_commands: VoiceCommandsSection,
    pub tts_cache_dir: std::path::PathBuf,
    /// Remaining app-config fields that aren't owned by a specific provider,
    /// kept so `save_config` can round-trip them without the GUI having to
    /// re-send every field on every save.
    pub kokoro: marginalia_config::KokoroSection,
    pub playback: marginalia_config::PlaybackSection,
    pub database_path: Option<std::path::PathBuf>,
    pub chunk_target_chars: Option<usize>,
    /// On-disk `marginalia.toml` path, supplied at runtime construction.
    /// `save_config` writes here. `None` = config is not backed by a file
    /// (in-memory runtime for tests).
    pub config_path: Option<std::path::PathBuf>,
    #[cfg(all(feature = "apple-stt", feature = "host-playback"))]
    pub aec_render_slot: AecRenderSlot,
}

impl ReconfigureContext {
    /// Build an `AppConfig` snapshot from the current context. Used by
    /// `save_config` to serialize the user's state back to disk.
    pub fn to_app_config(&self) -> marginalia_config::AppConfig {
        marginalia_config::AppConfig {
            database_path: self.database_path.clone(),
            tts_cache_dir: Some(self.tts_cache_dir.clone()),
            chunk_target_chars: self.chunk_target_chars,
            voice_commands: self.voice_commands.clone(),
            stt: self.stt.clone(),
            kokoro: self.kokoro.clone(),
            playback: self.playback.clone(),
            mlx: self.mlx.clone(),
        }
    }

    /// Persist the current context to `self.config_path` as canonical TOML.
    /// Callers that want to adjust app-level fields (e.g. `chunk_target_chars`,
    /// `voice_commands`) should mutate `self` first, then call this.
    pub fn save(&self) -> Result<(), String> {
        let path = self.config_path.as_deref().ok_or_else(|| {
            "ReconfigureContext has no config_path — the runtime was started without a file backing"
                .to_string()
        })?;
        self.to_app_config().write_to(path)
    }

    /// Build a `ProviderSpec` reflecting the currently-active configuration.
    pub fn current_spec(&self) -> ProviderSpec {
        ProviderSpec {
            // TTS backend choice is currently hard-wired at build time by
            // feature flags; when the Kokoro ONNX path is re-enabled we'll
            // surface that decision here. For now: MLX when `mlx-tts` is on,
            // otherwise kokoro/fake.
            tts_backend: default_tts_backend().to_string(),
            voice: self.mlx.voice.clone(),
            stt_engine: self.stt.engine.to_lowercase(),
            language: normalize_apple_language(&self.stt.language),
        }
    }
}

fn default_tts_backend() -> &'static str {
    #[cfg(feature = "mlx-tts")]
    {
        "mlx"
    }
    #[cfg(not(feature = "mlx-tts"))]
    {
        "kokoro"
    }
}

/// Apply `spec` to the running runtime. Returns an [`ApplyReport`] describing
/// which providers were actually swapped.
///
/// The function mutates `runtime`, `sidecar`, and `ctx` so that subsequent
/// calls diff against the new live state.
///
/// Errors bubble up from the provider constructors. On failure, the runtime
/// may be left with the old provider (if rebuilding failed early) or with no
/// provider of that kind (if the old one was dropped before the new one
/// failed to build); callers should treat any `Err` as "the user's choice
/// wasn't applied, show them the error".
pub fn apply_provider_spec(
    runtime: &mut SqliteRuntime,
    sidecar: &mut RuntimeSidecar,
    ctx: &mut ReconfigureContext,
    spec: &ProviderSpec,
) -> Result<ApplyReport, String> {
    let start = Instant::now();
    let current = ctx.current_spec();
    let mut report = ApplyReport::default();

    // ── TTS: voice or backend change ───────────────────────────────────
    if spec.voice != current.voice || spec.tts_backend != current.tts_backend {
        let new_tts = build_tts_provider(&spec.tts_backend, &spec.voice, ctx, &ctx.tts_cache_dir)?;
        if let Some(tts) = new_tts {
            runtime.set_speech_synthesizer_boxed(tts);
            runtime.set_default_voice(&spec.voice);
            ctx.mlx.voice = spec.voice.clone();
            report.tts_swapped = true;
        }
    }

    // ── STT: engine or language change ────────────────────────────────
    let stt_needs_rebuild =
        spec.stt_engine != current.stt_engine || spec.language != current.language;
    if stt_needs_rebuild {
        let normalized = normalize_apple_language(&Some(spec.language.clone()));
        ctx.stt.engine = spec.stt_engine.clone();
        ctx.stt.language = Some(normalized.clone());

        rebuild_stt(runtime, sidecar, ctx, &spec.stt_engine, &normalized)?;

        report.stt_swapped = true;
        report.language_changed = spec.language != current.language;
    }

    report.elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(report)
}

// ──────────────────────────────────────────────────────────────────────
// TTS rebuilding
// ──────────────────────────────────────────────────────────────────────

fn build_tts_provider(
    backend: &str,
    voice: &str,
    _ctx: &ReconfigureContext,
    tts_cache_dir: &Path,
) -> Result<Option<Box<dyn marginalia_core::ports::SpeechSynthesizer + Send>>, String> {
    match backend {
        #[cfg(feature = "mlx-tts")]
        "mlx" => {
            let synth = marginalia_tts_mlx::MlxSpeechSynthesizer::new(
                &_ctx.mlx.model,
                voice,
                tts_cache_dir,
            )?;
            Ok(Some(Box::new(synth)))
        }
        _ => {
            log::warn!(
                "reconfigure: TTS backend '{backend}' is not compiled in or is unimplemented"
            );
            let _ = (voice, tts_cache_dir);
            Ok(None)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────
// STT rebuilding
// ──────────────────────────────────────────────────────────────────────

fn rebuild_stt(
    runtime: &mut SqliteRuntime,
    sidecar: &mut RuntimeSidecar,
    ctx: &ReconfigureContext,
    engine: &str,
    language: &str,
) -> Result<(), String> {
    // Tear down Apple pipeline first (if any) — dropping these releases the
    // mic stream and the helper subprocess.
    #[cfg(feature = "apple-stt")]
    {
        sidecar.aec_pipeline = None;
        sidecar.waveform_data = None;
    }

    match engine {
        "apple" => {
            #[cfg(feature = "apple-stt")]
            {
                let commands = ctx.voice_commands.all_words();
                let cmd_silence = ctx.stt.commands.silence_timeout.unwrap_or(0.8);
                let dict_silence = ctx.stt.dictation.silence_timeout.unwrap_or(1.5);
                let dict_max = ctx.stt.dictation.max_record_seconds.unwrap_or(60.0);
                // Sibling of the TTS cache: recorded dictations live in
                // `<marginalia-data>/notes-audio/`, independent of the
                // TTS cache so cleaning one doesn't nuke the user's
                // voice memos. `tts_cache_dir.parent()` is the marginalia
                // data root; fall back to the cache dir itself on a
                // weird path with no parent.
                let notes_audio_dir = ctx
                    .tts_cache_dir
                    .parent()
                    .unwrap_or(&ctx.tts_cache_dir)
                    .join("notes-audio");
                let (rec, dict, aec_pipeline) = marginalia_stt_apple::new_apple_stt(
                    language,
                    commands,
                    cmd_silence,
                    dict_silence,
                    dict_max,
                    notes_audio_dir,
                )?;
                runtime.set_command_recognizer(rec);
                runtime.set_dictation_partial_slot(dict.dict_partial_slot());
                runtime.set_dictation_transcriber(dict);
                #[cfg(feature = "host-playback")]
                ctx.aec_render_slot.install(aec_pipeline.render_sender());
                sidecar.waveform_data = Some(aec_pipeline.waveform_data());
                sidecar.aec_pipeline = Some(aec_pipeline);
                Ok(())
            }
            #[cfg(not(feature = "apple-stt"))]
            {
                let _ = (runtime, sidecar, ctx, language);
                Err("engine=apple but apple-stt feature is not compiled in".to_string())
            }
        }
        "whisper" => {
            #[cfg(feature = "whisper-stt")]
            {
                let Some(model_path) = ctx.stt.whisper.model_path.clone() else {
                    return Err("[stt] engine=whisper but no model_path configured".to_string());
                };
                use marginalia_stt_whisper::{
                    WhisperCommandRecognizer, WhisperConfig, WhisperDictationTranscriber,
                };
                let whisper_lang = normalize_whisper_language(language);

                let mut cmd_cfg = WhisperConfig::new(&model_path);
                cmd_cfg.language = whisper_lang.clone();
                cmd_cfg.max_duration_seconds = ctx.stt.commands.max_record_seconds.unwrap_or(4.0);
                cmd_cfg.silence_timeout_seconds = ctx.stt.commands.silence_timeout.unwrap_or(0.8);
                if let Some(v) = ctx.stt.commands.speech_threshold {
                    cmd_cfg.speech_threshold = v;
                }

                let mut dict_cfg = WhisperConfig::new(&model_path);
                dict_cfg.language = whisper_lang;
                dict_cfg.max_duration_seconds = ctx.stt.dictation.max_record_seconds.unwrap_or(60.0);
                dict_cfg.silence_timeout_seconds = ctx.stt.dictation.silence_timeout.unwrap_or(1.5);
                if let Some(v) = ctx.stt.dictation.speech_threshold {
                    dict_cfg.speech_threshold = v;
                }

                let commands = ctx.voice_commands.all_words();
                let rec = WhisperCommandRecognizer::new(cmd_cfg, commands);
                let dict = WhisperDictationTranscriber::new(dict_cfg);

                runtime.set_stt_engine(crate::SttEngineOutput {
                    command_recognizer: Box::new(rec),
                    dictation_transcriber: Box::new(dict),
                    engine_label: "whisper".to_string(),
                });
                Ok(())
            }
            #[cfg(not(feature = "whisper-stt"))]
            {
                let _ = (runtime, sidecar, ctx, language);
                Err("engine=whisper but whisper-stt feature is not compiled in".to_string())
            }
        }
        other => Err(format!(
            "unknown STT engine '{other}' — valid: \"apple\", \"whisper\""
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_lang_normalization() {
        assert_eq!(normalize_apple_language(&None), "it-IT");
        assert_eq!(
            normalize_apple_language(&Some("it".to_string())),
            "it-IT"
        );
        assert_eq!(
            normalize_apple_language(&Some("en".to_string())),
            "en-US"
        );
        assert_eq!(
            normalize_apple_language(&Some("it-IT".to_string())),
            "it-IT"
        );
        assert_eq!(
            normalize_apple_language(&Some("fr-CA".to_string())),
            "fr-CA"
        );
    }

    #[test]
    fn whisper_lang_strips_region() {
        assert_eq!(normalize_whisper_language("it-IT"), "it");
        assert_eq!(normalize_whisper_language("en-US"), "en");
        assert_eq!(normalize_whisper_language("ja"), "ja");
    }

    #[test]
    fn apply_report_default_zero() {
        let r = ApplyReport::default();
        assert!(!r.tts_swapped);
        assert!(!r.stt_swapped);
        assert!(!r.language_changed);
        assert_eq!(r.elapsed_ms, 0);
    }
}
