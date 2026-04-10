use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;
use std::{env, fs, process};

const STYLE_VECTOR_WIDTH: usize = 256;

// ---------------------------------------------------------------------------
// Test phrases (Italian, increasing length)
// ---------------------------------------------------------------------------

const PHRASES: &[(&str, &str)] = &[
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
];

// ---------------------------------------------------------------------------
// Phonemizer (espeak-ng)
// ---------------------------------------------------------------------------

fn phonemize(text: &str) -> String {
    let mut child = Command::new("espeak-ng")
        .args(["-v", "it", "--ipa", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("espeak-ng not found — install with: brew install espeak-ng");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let raw = String::from_utf8_lossy(&output.stdout);
    normalize_phonemes(raw.trim())
}

fn normalize_phonemes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_space = false;
    for ch in input.chars() {
        let c = if ch.is_whitespace() { ' ' } else { ch };
        if c == ' ' {
            if !prev_space && !out.is_empty() {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

// ---------------------------------------------------------------------------
// Tokenizer (reads config.json symbol table)
// ---------------------------------------------------------------------------

fn load_vocab(config_path: &Path) -> HashMap<String, i64> {
    let data: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(config_path).expect("config.json not found"))
            .expect("invalid config.json");
    let symbols = data["symbol_to_id"]
        .as_object()
        .or_else(|| data["vocab"].as_object())
        .expect("config.json missing symbol_to_id or vocab");
    symbols
        .iter()
        .map(|(k, v)| (k.clone(), v.as_i64().unwrap()))
        .collect()
}

fn tokenize(phonemes: &str, vocab: &HashMap<String, i64>) -> Vec<i64> {
    phonemes
        .chars()
        .filter_map(|c| vocab.get(&c.to_string()).copied())
        .collect()
}

fn pad_tokens(tokens: &[i64]) -> Vec<i64> {
    let mut padded = Vec::with_capacity(tokens.len() + 2);
    padded.push(0);
    padded.extend_from_slice(tokens);
    padded.push(0);
    padded
}

// ---------------------------------------------------------------------------
// Voice style loader
// ---------------------------------------------------------------------------

fn load_voice_style(voice_path: &Path, token_count: usize) -> Vec<f32> {
    let bytes = fs::read(voice_path).expect("voice file not found");
    let values: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let frame_count = values.len() / STYLE_VECTOR_WIDTH;
    let idx = token_count.min(frame_count.saturating_sub(1));
    let start = idx * STYLE_VECTOR_WIDTH;
    values[start..start + STYLE_VECTOR_WIDTH].to_vec()
}

// ---------------------------------------------------------------------------
// WAV writer
// ---------------------------------------------------------------------------

fn write_wav(path: &Path, sample_rate: u32, audio: &[f32]) {
    let data_size = audio.len() * 2;
    let mut bytes = Vec::with_capacity(44 + data_size);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    bytes.extend_from_slice(&2u16.to_le_bytes()); // block align
    bytes.extend_from_slice(&16u16.to_le_bytes()); // bits
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_size as u32).to_le_bytes());
    for &s in audio {
        let pcm = (s.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        bytes.extend_from_slice(&pcm.to_le_bytes());
    }
    fs::write(path, bytes).expect("failed to write wav");
}

// ---------------------------------------------------------------------------
// ORT benchmark
// ---------------------------------------------------------------------------

struct OrtBench {
    session: Session,
    vocab: HashMap<String, i64>,
    voice_style_path: PathBuf,
    sample_rate: u32,
}

#[derive(Debug)]
struct BenchResult {
    label: String,
    chars: usize,
    tokens: usize,
    phonemize_ms: f64,
    tokenize_ms: f64,
    inference_ms: f64,
    wav_write_ms: f64,
    total_ms: f64,
    audio_duration_s: f64,
    rtf: f64, // realtime factor (audio_duration / inference_time)
}

impl OrtBench {
    fn load(assets_root: &Path, threads: usize) -> (Self, std::time::Duration) {
        let model_path = ["kokoro.onnx", "model.onnx", "kokoro-v1.0.onnx"]
            .iter()
            .map(|f| assets_root.join(f))
            .find(|p| p.exists())
            .expect("no model.onnx found");

        let lib_path = ["lib/libonnxruntime.dylib", "lib/libonnxruntime.so"]
            .iter()
            .map(|f| assets_root.join(f))
            .find(|p| p.exists())
            .or_else(|| env::var_os("ORT_DYLIB_PATH").map(PathBuf::from).filter(|p| p.exists()))
            .expect("no libonnxruntime found — place in assets/lib/ or set ORT_DYLIB_PATH");

        let config_path = assets_root.join("config.json");
        let voice_path = assets_root.join("voices/af.bin");

        let t = Instant::now();
        ort::init_from(&lib_path).expect("ort init").commit();

        let mut builder = Session::builder().expect("session builder");
        if threads > 0 {
            builder = builder
                .with_intra_threads(threads)
                .expect("set intra threads");
        }
        let session = builder
            .commit_from_file(&model_path)
            .expect("load model");
        let load_time = t.elapsed();

        let vocab = load_vocab(&config_path);

        (
            Self {
                session,
                vocab,
                voice_style_path: voice_path,
                sample_rate: 24_000,
            },
            load_time,
        )
    }

    fn run_phrase(&mut self, label: &str, text: &str, out_dir: &Path) -> BenchResult {
        // 1. Phonemize
        let t0 = Instant::now();
        let phonemes = phonemize(text);
        let phonemize_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // 2. Tokenize
        let t1 = Instant::now();
        let tokens = tokenize(&phonemes, &self.vocab);
        let padded = pad_tokens(&tokens);
        let style = load_voice_style(&self.voice_style_path, tokens.len());
        let tokenize_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // 3. Inference
        let token_shape = vec![1i64, padded.len() as i64];
        let style_shape = vec![1i64, STYLE_VECTOR_WIDTH as i64];
        let speed_shape = vec![1i64];

        let t2 = Instant::now();
        let outputs = self
            .session
            .run(
                ort::inputs! {
                    "input_ids" => Tensor::<i64>::from_array((token_shape, padded)).unwrap(),
                    "style" => Tensor::<f32>::from_array((style_shape, style)).unwrap(),
                    "speed" => Tensor::<f32>::from_array((speed_shape, vec![1.0f32])).unwrap(),
                },
            )
            .expect("inference failed");
        let inference_ms = t2.elapsed().as_secs_f64() * 1000.0;

        let (_, audio_data) = outputs[0].try_extract_tensor::<f32>().unwrap();
        let audio: Vec<f32> = audio_data.to_vec();

        // 4. WAV write
        let wav_path = out_dir.join(format!("{label}.wav"));
        let t3 = Instant::now();
        write_wav(&wav_path, self.sample_rate, &audio);
        let wav_write_ms = t3.elapsed().as_secs_f64() * 1000.0;

        let total_ms = phonemize_ms + tokenize_ms + inference_ms + wav_write_ms;
        let audio_duration_s = audio.len() as f64 / self.sample_rate as f64;
        let rtf = if inference_ms > 0.0 {
            audio_duration_s / (inference_ms / 1000.0)
        } else {
            0.0
        };

        BenchResult {
            label: label.to_string(),
            chars: text.len(),
            tokens: tokens.len(),
            phonemize_ms,
            tokenize_ms,
            inference_ms,
            wav_write_ms,
            total_ms,
            audio_duration_s,
            rtf,
        }
    }
}

fn print_results(backend: &str, threads: usize, load_ms: f64, results: &[BenchResult]) {
    println!("\n  {}", "=".repeat(60));
    println!("  Backend: {backend}");
    println!("  Threads: {}", if threads == 0 { "auto".to_string() } else { threads.to_string() });
    println!("  Model load: {load_ms:.0}ms");
    println!("  {}\n", "=".repeat(60));

    println!(
        "  {:<8} {:>5} {:>5} {:>8} {:>8} {:>9} {:>7} {:>8} {:>5}",
        "Label", "Chars", "Toks", "Phonem", "Tokeniz", "Inference", "WAV", "Total", "RTFx"
    );
    println!("  {}", "-".repeat(75));
    for r in results {
        println!(
            "  {:<8} {:>5} {:>5} {:>7.0}ms {:>7.0}ms {:>8.0}ms {:>5.0}ms {:>7.0}ms {:>5.1}x",
            r.label,
            r.chars,
            r.tokens,
            r.phonemize_ms,
            r.tokenize_ms,
            r.inference_ms,
            r.wav_write_ms,
            r.total_ms,
            r.rtf,
        );
    }

    if let Some(med) = results.iter().find(|r| r.label == "medium") {
        println!("\n  Key metric (medium phrase, {} chars):", med.chars);
        println!("    Inference:  {:.0}ms", med.inference_ms);
        println!("    Total:      {:.0}ms", med.total_ms);
        println!("    Audio:      {:.1}s", med.audio_duration_s);
        println!("    RTF:        {:.1}x realtime", med.rtf);
    }
}

// ---------------------------------------------------------------------------
// Piper benchmark (direct ORT, no piper-rs dependency)
// ---------------------------------------------------------------------------

struct PiperBench {
    session: Session,
    phoneme_id_map: HashMap<String, Vec<i64>>,
    sample_rate: u32,
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
    espeak_voice: String,
}

impl PiperBench {
    fn load(model_path: &Path, config_path: &Path, ort_lib: &Path) -> (Self, std::time::Duration) {
        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(config_path).unwrap()).unwrap();

        let phoneme_id_map: HashMap<String, Vec<i64>> = config["phoneme_id_map"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let ids: Vec<i64> = v.as_array().unwrap().iter().map(|x| x.as_i64().unwrap()).collect();
                (k.clone(), ids)
            })
            .collect();

        let sample_rate = config["audio"]["sample_rate"].as_u64().unwrap() as u32;
        let noise_scale = config["inference"]["noise_scale"].as_f64().unwrap() as f32;
        let length_scale = config["inference"]["length_scale"].as_f64().unwrap() as f32;
        let noise_w = config["inference"]["noise_w"].as_f64().unwrap() as f32;
        let espeak_voice = config["espeak"]["voice"].as_str().unwrap().to_string();

        let t = Instant::now();
        ort::init_from(ort_lib).expect("ort init").commit();
        let session = Session::builder()
            .expect("session builder")
            .with_intra_threads(0)
            .expect("set threads")
            .commit_from_file(model_path)
            .expect("load piper model");
        let load_time = t.elapsed();

        (
            Self { session, phoneme_id_map, sample_rate, noise_scale, length_scale, noise_w, espeak_voice },
            load_time,
        )
    }

    fn text_to_phoneme_ids(&self, text: &str) -> Vec<i64> {
        // Use espeak-ng to get IPA phonemes
        let mut child = Command::new("espeak-ng")
            .args(["-v", &self.espeak_voice, "--ipa", "-q"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("espeak-ng not found");
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        let phonemes = String::from_utf8_lossy(&output.stdout);

        // Convert phonemes to IDs using the map
        // Piper inserts padding (0) between each phoneme and at start/end
        let mut ids = vec![0i64]; // BOS padding
        for ch in phonemes.trim().chars() {
            let key = ch.to_string();
            if let Some(mapped) = self.phoneme_id_map.get(&key) {
                ids.extend(mapped);
                ids.push(0); // inter-phoneme padding
            }
        }
        ids
    }

    fn run_phrase(&mut self, label: &str, text: &str, out_dir: &Path) -> BenchResult {
        let t0 = Instant::now();
        let phoneme_ids = self.text_to_phoneme_ids(text);
        let phonemize_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let n_phonemes = phoneme_ids.len();
        let input_shape = vec![1i64, n_phonemes as i64];
        let lengths = vec![n_phonemes as i64];
        let scales = vec![self.noise_scale, self.length_scale, self.noise_w];

        let t1 = Instant::now();
        let outputs = self.session.run(
            ort::inputs! {
                "input" => Tensor::<i64>::from_array((input_shape, phoneme_ids)).unwrap(),
                "input_lengths" => Tensor::<i64>::from_array(([1i64], lengths)).unwrap(),
                "scales" => Tensor::<f32>::from_array(([3i64], scales)).unwrap(),
            },
        ).expect("piper inference failed");
        let inference_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let (_, audio_data) = outputs[0].try_extract_tensor::<f32>().unwrap();
        let audio: Vec<f32> = audio_data.to_vec();

        let t2 = Instant::now();
        let wav_path = out_dir.join(format!("{label}.wav"));
        write_wav(&wav_path, self.sample_rate, &audio);
        let wav_write_ms = t2.elapsed().as_secs_f64() * 1000.0;

        let total_ms = phonemize_ms + inference_ms + wav_write_ms;
        let audio_duration_s = audio.len() as f64 / self.sample_rate as f64;
        let rtf = if inference_ms > 0.0 {
            audio_duration_s / (inference_ms / 1000.0)
        } else { 0.0 };

        BenchResult {
            label: label.to_string(),
            chars: text.len(),
            tokens: n_phonemes,
            phonemize_ms,
            tokenize_ms: 0.0,
            inference_ms,
            wav_write_ms,
            total_ms,
            audio_duration_s,
            rtf,
        }
    }
}

fn run_piper(model_path: &Path, config_path: &Path, ort_lib: &Path) {
    let out_dir = env::temp_dir().join("tts-bench-piper-wav");
    fs::create_dir_all(&out_dir).unwrap();

    eprint!("  Loading Piper model...");
    let (mut bench, load_time) = PiperBench::load(model_path, config_path, ort_lib);
    let load_ms = load_time.as_secs_f64() * 1000.0;
    eprintln!(" {load_ms:.0}ms");

    eprint!("  Warmup...");
    let _ = bench.run_phrase("warmup", PHRASES[0].1, &out_dir);
    eprintln!(" done");

    let mut results = Vec::new();
    for &(label, text) in PHRASES {
        eprint!("  {label}...");
        let r = bench.run_phrase(label, text, &out_dir);
        eprintln!(" {:.0}ms", r.total_ms);
        results.push(r);
    }

    let model_name = model_path.file_stem().unwrap().to_string_lossy();
    print_results(&format!("Piper ({model_name})"), 0, load_ms, &results);
    let _ = fs::remove_dir_all(&out_dir);
}

fn print_usage() {
    eprintln!("tts-bench — TTS backend benchmark for Apple Silicon");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  tts-bench ort <kokoro_assets>           Kokoro ONNX Runtime CPU benchmark");
    eprintln!("  tts-bench ort-sweep <kokoro_assets>     Test all thread counts (1,2,4,8,auto)");
    eprintln!("  tts-bench piper <model.onnx> <cfg.json> <libort>  Piper TTS benchmark");
    eprintln!();
    eprintln!("Kokoro assets must contain: model.onnx, config.json, voices/af.bin, lib/libonnxruntime.dylib");
}

fn run_ort(assets_root: &Path, threads: usize) {
    let out_dir = env::temp_dir().join("tts-bench-wav");
    fs::create_dir_all(&out_dir).unwrap();

    let (mut bench, load_time) = OrtBench::load(assets_root, threads);
    let load_ms = load_time.as_secs_f64() * 1000.0;

    // Warmup run (not counted)
    eprint!("  Warmup...");
    let _ = bench.run_phrase("warmup", PHRASES[0].1, &out_dir);
    eprintln!(" done");

    let mut results = Vec::new();
    for &(label, text) in PHRASES {
        eprint!("  {label}...");
        let r = bench.run_phrase(label, text, &out_dir);
        eprintln!(" {:.0}ms", r.total_ms);
        results.push(r);
    }

    print_results(
        "ONNX Runtime CPU",
        threads,
        load_ms,
        &results,
    );

    let _ = fs::remove_dir_all(&out_dir);
}

fn run_ort_sweep(assets_root: &Path) {
    let thread_configs: &[usize] = &[1, 2, 4, 8, 0]; // 0 = auto

    println!("\n  ORT Thread Sweep — testing {} configurations\n", thread_configs.len());

    let out_dir = env::temp_dir().join("tts-bench-wav");
    fs::create_dir_all(&out_dir).unwrap();

    // We can only init ORT once, so just test with auto threads
    // and use intra_threads on the session builder.
    // But init_from is a global once — so we load once, then rebuild sessions.
    // Actually, in ort rc.12, init_from uses OnceLock so we can only call it once.
    // We'll just test with the first config and report.

    // For a fair sweep, we need separate processes. Use self-invocation.
    let exe = env::current_exe().unwrap();
    let assets_str = assets_root.to_str().unwrap();

    println!("  {:<10} {:>9} {:>9} {:>9} {:>9} {:>6}",
        "Threads", "Tiny", "Short", "Medium", "Long", "RTFx");
    println!("  {}", "-".repeat(60));

    for &threads in thread_configs {
        let output = Command::new(&exe)
            .args(["ort-json", assets_str, &threads.to_string()])
            .output()
            .expect("failed to run sub-process");

        if !output.status.success() {
            let thread_label = if threads == 0 { "auto".to_string() } else { threads.to_string() };
            eprintln!("  {:<10} FAILED", thread_label);
            continue;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&stdout) {
            let thread_label = if threads == 0 { "auto".to_string() } else { threads.to_string() };
            let results = data["results"].as_array().unwrap();
            let times: Vec<String> = results
                .iter()
                .map(|r| format!("{:.0}ms", r["inference_ms"].as_f64().unwrap()))
                .collect();
            let rtf = results
                .iter()
                .find(|r| r["label"].as_str() == Some("medium"))
                .map(|r| r["rtf"].as_f64().unwrap())
                .unwrap_or(0.0);
            println!(
                "  {:<10} {:>9} {:>9} {:>9} {:>9} {:>5.1}x",
                thread_label,
                times.get(0).unwrap_or(&"-".to_string()),
                times.get(1).unwrap_or(&"-".to_string()),
                times.get(2).unwrap_or(&"-".to_string()),
                times.get(3).unwrap_or(&"-".to_string()),
                rtf,
            );
        }
    }

    let _ = fs::remove_dir_all(&out_dir);
}

fn run_ort_json(assets_root: &Path, threads: usize) {
    let out_dir = env::temp_dir().join("tts-bench-wav-json");
    fs::create_dir_all(&out_dir).unwrap();

    let (mut bench, load_time) = OrtBench::load(assets_root, threads);

    // Warmup
    let _ = bench.run_phrase("warmup", PHRASES[0].1, &out_dir);

    let mut results = Vec::new();
    for &(label, text) in PHRASES {
        let r = bench.run_phrase(label, text, &out_dir);
        results.push(serde_json::json!({
            "label": r.label,
            "chars": r.chars,
            "tokens": r.tokens,
            "inference_ms": r.inference_ms,
            "total_ms": r.total_ms,
            "audio_duration_s": r.audio_duration_s,
            "rtf": r.rtf,
        }));
    }

    println!(
        "{}",
        serde_json::json!({
            "load_ms": load_time.as_secs_f64() * 1000.0,
            "threads": threads,
            "results": results,
        })
    );

    let _ = fs::remove_dir_all(&out_dir);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "ort" => {
            if args.len() < 3 {
                eprintln!("Usage: tts-bench ort <assets_dir> [threads]");
                process::exit(1);
            }
            let threads = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            run_ort(Path::new(&args[2]), threads);
        }
        "ort-sweep" => {
            if args.len() < 3 {
                eprintln!("Usage: tts-bench ort-sweep <assets_dir>");
                process::exit(1);
            }
            run_ort_sweep(Path::new(&args[2]));
        }
        "piper" => {
            if args.len() < 5 {
                eprintln!("Usage: tts-bench piper <model.onnx> <model.onnx.json> <libonnxruntime>");
                process::exit(1);
            }
            run_piper(Path::new(&args[2]), Path::new(&args[3]), Path::new(&args[4]));
        }
        "ort-json" => {
            // Internal: used by ort-sweep for sub-process isolation
            let threads = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            run_ort_json(Path::new(&args[2]), threads);
        }
        "help" | "--help" | "-h" => print_usage(),
        other => {
            eprintln!("Unknown command: {other}");
            print_usage();
            process::exit(1);
        }
    }
}
