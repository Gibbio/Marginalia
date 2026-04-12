//! `RuntimeBuilder` — replaces manual provider wiring in each app.
//!
//! Before: every app had ~500 lines of platform detection, provider
//! construction, AEC wiring, and session restore.
//! Now: `RuntimeBuilder::new(db_path).stt(stt_cfg).build()?`.

use crate::{RuntimeConfig, SqliteRuntime};

#[cfg(feature = "apple-stt")]
pub use marginalia_stt_apple::aec_pipeline::WaveformData;
use marginalia_config::{
    KokoroSection, MlxSection, PlaybackSection, SttSection, VoiceCommandsSection,
};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Side resources that must outlive the runtime but can't live inside it
/// (e.g. `cpal::Stream` is `!Send` and `SqliteRuntime` is behind `Arc<Mutex>`).
pub struct RuntimeSidecar {
    #[cfg(feature = "apple-stt")]
    pub waveform_data: Option<
        std::sync::Arc<
            std::sync::Mutex<marginalia_stt_apple::aec_pipeline::WaveformData>,
        >,
    >,
    #[cfg(not(feature = "apple-stt"))]
    _private: (),
}

pub struct BuildOutput {
    pub runtime: SqliteRuntime,
    pub sidecar: RuntimeSidecar,
    pub tts_label: String,
    pub stt_label: String,
    pub dictation_label: String,
    pub playback_label: String,
    pub stt_debug: bool,
    pub voice_commands: VoiceCommandsSection,
}

pub struct RuntimeBuilder {
    db_path: PathBuf,
    config: RuntimeConfig,
    voice_commands: VoiceCommandsSection,
    stt: SttSection,
    kokoro: KokoroSection,
    mlx: MlxSection,
    playback: PlaybackSection,
}

impl RuntimeBuilder {
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
            config: RuntimeConfig::default(),
            voice_commands: VoiceCommandsSection::default(),
            stt: SttSection::default(),
            kokoro: KokoroSection::default(),
            mlx: MlxSection::default(),
            playback: PlaybackSection::default(),
        }
    }

    pub fn config(mut self, config: RuntimeConfig) -> Self {
        self.config = config;
        self
    }

    pub fn voice_commands(mut self, vc: VoiceCommandsSection) -> Self {
        self.voice_commands = vc;
        self
    }

    pub fn stt(mut self, stt: SttSection) -> Self {
        self.stt = stt;
        self
    }

    pub fn kokoro(mut self, kokoro: KokoroSection) -> Self {
        self.kokoro = kokoro;
        self
    }

    pub fn mlx(mut self, mlx: MlxSection) -> Self {
        self.mlx = mlx;
        self
    }

    pub fn playback(mut self, playback: PlaybackSection) -> Self {
        self.playback = playback;
        self
    }

    pub fn build(self) -> Result<BuildOutput, String> {
        if let Some(parent) = self.db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let tts_cache_dir = self.config.tts_cache_dir.clone().unwrap_or_else(|| {
            self.db_path
                .parent()
                .unwrap_or(Path::new("."))
                .join("tts-cache")
        });
        std::fs::create_dir_all(&tts_cache_dir).ok();

        let mut config = self.config;
        config.tts_cache_dir = Some(tts_cache_dir.clone());

        let mut runtime = SqliteRuntime::open_with_config(&self.db_path, config)
            .map_err(|e| format!("Unable to open runtime database: {e}"))?;

        // ── Playback (created early, AEC callback wired later) ──
        #[cfg(feature = "host-playback")]
        let mut playback_engine = if self.playback.fake {
            None
        } else {
            Some(marginalia_playback_host::HostPlaybackEngine::default())
        };
        #[cfg(not(feature = "host-playback"))]
        let playback_engine: Option<()> = None;
        let playback_label = if playback_engine.is_some() {
            "host"
        } else {
            "fake"
        };

        // ── TTS ──
        let mut tts_label = "fake";

        #[allow(unused_variables)]
        let kokoro = &self.kokoro;
        if let Some(assets_root) = &kokoro.assets_root {
            let kokoro_config =
                marginalia_tts_kokoro::KokoroConfig::from_assets_root(assets_root);
            let readiness = kokoro_config.readiness_report();
            if readiness.is_ready() {
                let synth_config =
                    marginalia_tts_kokoro::KokoroSpeechSynthesizerConfig::new(&tts_cache_dir);
                let synthesizer = if let Some(program) = &kokoro.phonemizer_program {
                    let args = if kokoro.phonemizer_args.is_empty() {
                        vec![
                            "-v".to_string(),
                            "it".to_string(),
                            "--ipa".to_string(),
                            "-q".to_string(),
                        ]
                    } else {
                        kokoro.phonemizer_args.clone()
                    };
                    let text_processor =
                        marginalia_tts_kokoro::KokoroTextProcessor::with_external_command(
                            kokoro_config.clone(),
                            marginalia_tts_kokoro::KokoroExternalPhonemizerConfig {
                                program: program.clone(),
                                args,
                            },
                        );
                    marginalia_tts_kokoro::KokoroSpeechSynthesizer::with_text_processor(
                        kokoro_config,
                        synth_config,
                        text_processor,
                    )
                } else {
                    marginalia_tts_kokoro::KokoroSpeechSynthesizer::new(
                        kokoro_config,
                        synth_config,
                    )
                };
                runtime.set_speech_synthesizer(synthesizer);
                tts_label = "kokoro";
            }
        }

        #[cfg(feature = "mlx-tts")]
        {
            match marginalia_tts_mlx::MlxSpeechSynthesizer::new(
                &self.mlx.model,
                &self.mlx.voice,
                &tts_cache_dir,
            ) {
                Ok(synth) => {
                    runtime.set_speech_synthesizer(synth);
                    runtime.set_default_voice(&self.mlx.voice);
                    tts_label = "kokoro-mlx";
                }
                Err(e) => {
                    log::warn!("MLX TTS init failed, keeping {tts_label}: {e}");
                }
            }
        }

        // ── STT ──
        let mut stt_label = "fake";
        let mut dictation_label = "fake";
        #[cfg(feature = "apple-stt")]
        let mut _waveform_data: Option<
            std::sync::Arc<
                std::sync::Mutex<marginalia_stt_apple::aec_pipeline::WaveformData>,
            >,
        > = None;

        let stt_engine = self.stt.engine.to_lowercase();

        if stt_engine == "apple" {
            #[cfg(feature = "apple-stt")]
            {
                let commands = self.voice_commands.all_words();
                let language = match self.stt.language.as_deref() {
                    None => "it-IT".to_string(),
                    Some(l) if l.contains('-') => l.to_string(),
                    Some(l) if l.eq_ignore_ascii_case("it") => "it-IT".to_string(),
                    Some(l) if l.eq_ignore_ascii_case("en") => "en-US".to_string(),
                    Some(l) => l.to_string(),
                };
                let cmd_silence = self.stt.commands.silence_timeout.unwrap_or(0.8);
                let dict_silence = self.stt.dictation.silence_timeout.unwrap_or(1.5);
                let dict_max = self.stt.dictation.max_record_seconds.unwrap_or(60.0);
                match marginalia_stt_apple::new_apple_stt(
                    &language,
                    commands,
                    cmd_silence,
                    dict_silence,
                    dict_max,
                ) {
                    Ok((rec, dict, aec_pipeline)) => {
                        runtime.set_command_recognizer(rec);
                        runtime.set_dictation_transcriber(dict);
                        let render_tx = aec_pipeline.render_sender();
                        #[cfg(feature = "host-playback")]
                        if let Some(ref mut pe) = playback_engine {
                            pe.set_play_samples_callback(Box::new(move |samples| {
                                use marginalia_stt_apple::aec_pipeline::RenderCommand;
                                let _ =
                                    render_tx.try_send(RenderCommand::SetReference(samples));
                            }));
                        }
                        _waveform_data = Some(aec_pipeline.waveform_data());
                        Box::leak(Box::new(aec_pipeline));
                        runtime.set_provider_doctor_blob(
                            "apple_stt",
                            json!({
                                "ready": true,
                                "language": language,
                                "cmd_silence_timeout": cmd_silence,
                                "dict_silence_timeout": dict_silence,
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
            if let Some(model_path) = self.stt.whisper.model_path.clone() {
                use marginalia_stt_whisper::{
                    WhisperCommandRecognizer, WhisperConfig, WhisperDictationTranscriber,
                };
                let language = self
                    .stt
                    .language
                    .clone()
                    .map(|l| l.split('-').next().unwrap_or("it").to_string())
                    .unwrap_or_else(|| "it".to_string());

                let mut cmd_cfg = WhisperConfig::new(&model_path);
                cmd_cfg.language = language.clone();
                cmd_cfg.max_duration_seconds = 4.0;
                cmd_cfg.silence_timeout_seconds = 0.8;
                if let Some(v) = self.stt.commands.silence_timeout {
                    cmd_cfg.silence_timeout_seconds = v;
                }
                if let Some(v) = self.stt.commands.max_record_seconds {
                    cmd_cfg.max_duration_seconds = v;
                }
                if let Some(v) = self.stt.commands.speech_threshold {
                    cmd_cfg.speech_threshold = v;
                }
                let cmd_commands = self.voice_commands.all_words();
                let whisper_cmd_rec =
                    WhisperCommandRecognizer::new(cmd_cfg, cmd_commands);

                let mut dict_cfg = WhisperConfig::new(&model_path);
                dict_cfg.language = language;
                if let Some(v) = self.stt.dictation.silence_timeout {
                    dict_cfg.silence_timeout_seconds = v;
                }
                if let Some(v) = self.stt.dictation.max_record_seconds {
                    dict_cfg.max_duration_seconds = v;
                }
                if let Some(v) = self.stt.dictation.speech_threshold {
                    dict_cfg.speech_threshold = v;
                }
                let whisper_dict = WhisperDictationTranscriber::new(dict_cfg.clone());

                runtime.set_stt_engine(crate::SttEngineOutput {
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
                        "max_record_seconds": dict_cfg.max_duration_seconds,
                        "silence_timeout": dict_cfg.silence_timeout_seconds,
                    }),
                );
                dictation_label = "whisper";
            }
            #[cfg(not(feature = "whisper-stt"))]
            log::warn!("[stt] engine=whisper but whisper-stt feature is not built in");
        } else {
            log::error!(
                "[stt] unknown engine '{stt_engine}' — valid: \"apple\", \"whisper\""
            );
        }

        // ── Hand playback to runtime ──
        #[cfg(feature = "host-playback")]
        if let Some(pe) = playback_engine {
            runtime.set_playback_engine(pe);
        }

        // ── Restore session ──
        if let Some(session) = runtime.restore_session() {
            log::info!(
                "Restored session: document={} section={} chunk={}",
                session.document_id,
                session.position.section_index,
                session.position.chunk_index,
            );
        }

        Ok(BuildOutput {
            runtime,
            sidecar: RuntimeSidecar {
                #[cfg(feature = "apple-stt")]
                waveform_data: _waveform_data,
                #[cfg(not(feature = "apple-stt"))]
                _private: (),
            },
            tts_label: tts_label.to_string(),
            stt_label: stt_label.to_string(),
            dictation_label: dictation_label.to_string(),
            playback_label: playback_label.to_string(),
            stt_debug: self.stt.debug,
            voice_commands: self.voice_commands,
        })
    }
}
