use marginalia_core::domain::build_document_from_import;
use marginalia_core::ports::{DocumentImporter, SpeechSynthesizer, SynthesisRequest};
use marginalia_import_text::TextDocumentImporter;
use marginalia_runtime::SqliteRuntime;
use marginalia_tts_kokoro::{
    KokoroConfig, KokoroExternalPhonemizerConfig, KokoroSpeechSynthesizer,
    KokoroSpeechSynthesizerConfig, KokoroTextProcessor,
};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{env, process};

fn db_path() -> PathBuf {
    let dir = dirs_or_default();
    dir.join("marginalia-cli.db")
}

fn dirs_or_default() -> PathBuf {
    env::var("MARGINALIA_DB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn open_runtime() -> SqliteRuntime {
    let path = db_path();
    SqliteRuntime::open(&path).unwrap_or_else(|e| {
        eprintln!("Errore apertura database {}: {e}", path.display());
        process::exit(1);
    })
}

fn cmd_ingest(runtime: &mut SqliteRuntime, file_path: &str) {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!("File non trovato: {file_path}");
        process::exit(1);
    }
    match runtime.ingest_path(path) {
        Ok(outcome) => {
            if outcome.already_present {
                println!("Documento gia' presente: {}", outcome.document.document_id);
            } else {
                println!("Documento importato: {}", outcome.document.title);
                println!("  ID:       {}", outcome.document.document_id);
                println!("  Capitoli: {}", outcome.stats.chapter_count);
                println!("  Chunks:   {}", outcome.stats.chunk_count);
                println!("  Caratteri:{}", outcome.stats.raw_char_count);
            }
        }
        Err(e) => {
            eprintln!("Errore durante l'importazione: {e}");
            process::exit(1);
        }
    }
}

fn cmd_list(runtime: &SqliteRuntime) {
    let docs = runtime.list_documents();
    if docs.is_empty() {
        println!("Nessun documento. Usa 'ingest <file>' per importarne uno.");
        return;
    }
    println!(
        "{:<14} {:<6} {:<6} {:<20}",
        "ID", "Cap.", "Chunks", "Titolo"
    );
    println!("{}", "-".repeat(60));
    for doc in &docs {
        println!(
            "{:<14} {:<6} {:<6} {}",
            doc.document_id, doc.chapter_count, doc.chunk_count, doc.title
        );
    }
}

fn cmd_read(runtime: &SqliteRuntime, document_id: &str) {
    let view = match runtime.document_view(Some(document_id)) {
        Some(v) => v,
        None => {
            eprintln!("Documento non trovato: {document_id}");
            process::exit(1);
        }
    };

    println!("=== {} ===", view.title);
    println!(
        "ID: {}  |  {} capitoli, {} chunks\n",
        view.document_id, view.chapter_count, view.chunk_count
    );

    for section in &view.sections {
        println!("--- {} ---", section.title);
        for chunk in &section.chunks {
            println!("{}", chunk.text);
        }
        println!();
    }
}

fn cmd_show(runtime: &SqliteRuntime, document_id: &str) {
    let view = match runtime.document_view(Some(document_id)) {
        Some(v) => v,
        None => {
            eprintln!("Documento non trovato: {document_id}");
            process::exit(1);
        }
    };

    println!("Titolo:   {}", view.title);
    println!("ID:       {}", view.document_id);
    println!("Sorgente: {}", view.source_path);
    println!("Capitoli: {}", view.chapter_count);
    println!("Chunks:   {}", view.chunk_count);
    println!();
    println!("Indice:");
    for section in &view.sections {
        println!(
            "  [{}] {} ({} chunks)",
            section.index, section.title, section.chunk_count
        );
    }
}

// ---------------------------------------------------------------------------
// Benchmark commands
// ---------------------------------------------------------------------------

fn cmd_bench_ingest(file_path: &str) {
    let path = Path::new(file_path);
    if !path.exists() {
        eprintln!("File non trovato: {file_path}");
        process::exit(1);
    }

    println!("bench-ingest: {file_path}\n");

    // 1. Import (parsing file -> ImportedDocument)
    let importer = TextDocumentImporter;
    let t0 = Instant::now();
    let imported = importer.import_path(path).unwrap_or_else(|e| {
        eprintln!("Errore import: {e}");
        process::exit(1);
    });
    let import_time = t0.elapsed();

    let title = imported
        .title
        .clone()
        .unwrap_or_else(|| "(senza titolo)".to_string());
    let section_count = imported.sections.len();
    let raw_chars: usize = imported
        .sections
        .iter()
        .flat_map(|s| &s.paragraphs)
        .map(|p| p.len())
        .sum();

    println!("  Import (parsing):");
    println!("    Titolo:    {title}");
    println!("    Sezioni:   {section_count}");
    println!("    Caratteri: {raw_chars}");
    println!("    Tempo:     {import_time:?}");

    // 2. Chunking (ImportedDocument -> Document)
    let t1 = Instant::now();
    let document = build_document_from_import(imported, 300);
    let chunk_time = t1.elapsed();

    let chunk_count = document.total_chunk_count();
    println!("\n  Chunking:");
    println!("    Chunks:    {chunk_count}");
    println!("    Tempo:     {chunk_time:?}");

    // 3. SQLite storage (save + retrieve)
    let t2 = Instant::now();
    let mut runtime =
        SqliteRuntime::open_in_memory().expect("impossibile aprire database in-memory");
    let db_open_time = t2.elapsed();

    let doc_id = document.document_id.clone();

    let t3 = Instant::now();
    runtime.ingest_path(path).expect("ingest fallito");
    let full_ingest_time = t3.elapsed();

    let t4 = Instant::now();
    let _view = runtime.document_view(Some(&doc_id));
    let retrieve_time = t4.elapsed();

    println!("\n  Storage SQLite (in-memory):");
    println!("    Apertura DB:  {db_open_time:?}");
    println!("    Ingest full:  {full_ingest_time:?}");
    println!("    Retrieve:     {retrieve_time:?}");

    // Total
    let total = import_time + chunk_time + full_ingest_time;
    println!("\n  Totale pipeline: {total:?}");
}

fn cmd_bench_tts(document_id: &str, max_chunks: Option<usize>) {
    let assets_root = env::var("MARGINALIA_KOKORO_ASSETS").unwrap_or_else(|_| {
        eprintln!("MARGINALIA_KOKORO_ASSETS non impostata.");
        eprintln!("Punta alla directory con modello Kokoro ONNX e voci.");
        process::exit(1);
    });

    let runtime = open_runtime();
    let view = match runtime.document_view(Some(document_id)) {
        Some(v) => v,
        None => {
            eprintln!("Documento non trovato: {document_id}");
            process::exit(1);
        }
    };

    let kokoro_config = KokoroConfig::from_assets_root(&assets_root);
    // Verifica solo che i file esistano (senza probe_onnx_runtime che
    // chiama init_from e causa un deadlock OnceLock alla seconda chiamata).
    let readiness = kokoro_config.readiness_report();
    if !readiness.is_ready() {
        eprintln!("Kokoro non pronto. Verifica gli asset in: {assets_root}");
        eprintln!("  Modello:  {:?}", readiness.model_path);
        eprintln!("  Config:   {:?}", readiness.config_path);
        eprintln!("  Voce:     {:?}", readiness.voice_path);
        process::exit(1);
    }

    let tts_cache = dirs_or_default().join(".marginalia-bench-tts-cache");
    let synth_config = KokoroSpeechSynthesizerConfig::new(&tts_cache);

    let phonemizer_program =
        env::var("MARGINALIA_PHONEMIZER").unwrap_or_else(|_| "espeak-ng".to_string());
    let phonemizer_args = env::var("MARGINALIA_PHONEMIZER_ARGS")
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_else(|_| vec!["-v".into(), "it".into(), "--ipa".into(), "-q".into()]);

    let text_processor = KokoroTextProcessor::with_external_command(
        kokoro_config.clone(),
        KokoroExternalPhonemizerConfig {
            program: phonemizer_program.clone(),
            args: phonemizer_args,
        },
    );
    let mut synthesizer = KokoroSpeechSynthesizer::with_text_processor(
        kokoro_config.clone(),
        synth_config,
        text_processor,
    );

    let language = runtime.config().default_language.clone();
    let voice =
        env::var("MARGINALIA_VOICE").unwrap_or_else(|_| kokoro_config.default_voice.clone());

    // Warm up: carica il modello ONNX (puo' richiedere qualche secondo)
    eprint!("  Caricamento modello ONNX...");
    let t_warmup = Instant::now();
    if let Err(e) = synthesizer.warm_up() {
        eprintln!(" ERRORE: {e}");
        process::exit(1);
    }
    eprintln!(" ok ({:?})", t_warmup.elapsed());

    // Collect all chunks
    let chunks: Vec<(usize, usize, &str)> = view
        .sections
        .iter()
        .flat_map(|s| {
            s.chunks
                .iter()
                .map(move |c| (s.index, c.index, c.text.as_str()))
        })
        .collect();

    let limit = max_chunks.unwrap_or(chunks.len()).min(chunks.len());

    println!("bench-tts: \"{}\" ({document_id})", view.title);
    println!(
        "  Chunks totali: {}  |  Da sintetizzare: {limit}",
        chunks.len()
    );
    println!("  Voce: {voice}  |  Lingua: {language}");
    println!("  Assets: {assets_root}");
    println!();
    println!(
        "  {:<8} {:<8} {:<10} {:<10} {:<8} {:<10}",
        "Sez.", "Chunk", "Caratteri", "Tempo", "Bytes", "Antepr."
    );
    println!("  {}", "-".repeat(70));

    let mut total_chars = 0usize;
    let mut total_bytes = 0usize;
    let mut chunk_times = Vec::with_capacity(limit);

    for &(sec_idx, chunk_idx, text) in chunks.iter().take(limit) {
        let char_count = text.len();
        let preview: String = text.chars().take(40).collect();

        let t = Instant::now();
        let result = synthesizer.synthesize(SynthesisRequest {
            text: text.to_string(),
            voice: Some(voice.clone()),
            language: language.clone(),
        });
        let elapsed = t.elapsed();

        match result {
            Ok(synth) => {
                total_chars += char_count;
                total_bytes += synth.byte_length;
                chunk_times.push(elapsed);

                println!(
                    "  {:<8} {:<8} {:<10} {:<10} {:<8} {}...",
                    sec_idx,
                    chunk_idx,
                    char_count,
                    format!("{elapsed:?}"),
                    synth.byte_length,
                    preview,
                );
            }
            Err(e) => {
                chunk_times.push(elapsed);
                println!(
                    "  {:<8} {:<8} {:<10} ERRORE: {}",
                    sec_idx, chunk_idx, char_count, e,
                );
            }
        }
    }

    println!();
    let total_time: std::time::Duration = chunk_times.iter().sum();
    let avg_time = if chunk_times.is_empty() {
        std::time::Duration::ZERO
    } else {
        total_time / chunk_times.len() as u32
    };
    let min_time = chunk_times.iter().min().copied().unwrap_or_default();
    let max_time = chunk_times.iter().max().copied().unwrap_or_default();

    println!("  Riepilogo:");
    println!("    Chunks sintetizzati: {}", chunk_times.len());
    println!("    Caratteri totali:    {total_chars}");
    println!("    Bytes audio totali:  {total_bytes}");
    println!("    Tempo totale:        {total_time:?}");
    println!("    Tempo medio/chunk:   {avg_time:?}");
    println!("    Min:                 {min_time:?}");
    println!("    Max:                 {max_time:?}");
    if total_chars > 0 && !total_time.is_zero() {
        let chars_per_sec = total_chars as f64 / total_time.as_secs_f64();
        println!("    Throughput:          {chars_per_sec:.0} char/s");
    }

    // Cleanup cache
    let _ = std::fs::remove_dir_all(&tts_cache);
}

fn print_usage() {
    eprintln!("marginalia-cli - test CLI per Marginalia core");
    eprintln!();
    eprintln!("Uso:");
    eprintln!("  marginalia-cli ingest <file.txt|file.md>       Importa un documento");
    eprintln!("  marginalia-cli list                            Lista documenti importati");
    eprintln!("  marginalia-cli show <document_id>              Mostra info documento");
    eprintln!("  marginalia-cli read <document_id>              Leggi contenuto documento");
    eprintln!();
    eprintln!("Benchmark:");
    eprintln!("  marginalia-cli bench-ingest <file>             Misura tempi pipeline ingest");
    eprintln!("  marginalia-cli bench-tts <document_id> [N]     Misura tempi sintesi TTS (primi N chunks)");
    eprintln!();
    eprintln!("Variabili d'ambiente:");
    eprintln!("  MARGINALIA_DB_DIR           Directory per il database (default: .)");
    eprintln!("  MARGINALIA_KOKORO_ASSETS    Directory asset Kokoro (per bench-tts)");
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "ingest" => {
            if args.len() < 3 {
                eprintln!("Uso: marginalia-cli ingest <file>");
                process::exit(1);
            }
            let mut runtime = open_runtime();
            cmd_ingest(&mut runtime, &args[2]);
        }
        "list" => {
            let runtime = open_runtime();
            cmd_list(&runtime);
        }
        "show" => {
            if args.len() < 3 {
                eprintln!("Uso: marginalia-cli show <document_id>");
                process::exit(1);
            }
            let runtime = open_runtime();
            cmd_show(&runtime, &args[2]);
        }
        "read" => {
            if args.len() < 3 {
                eprintln!("Uso: marginalia-cli read <document_id>");
                process::exit(1);
            }
            let runtime = open_runtime();
            cmd_read(&runtime, &args[2]);
        }
        "bench-ingest" => {
            if args.len() < 3 {
                eprintln!("Uso: marginalia-cli bench-ingest <file>");
                process::exit(1);
            }
            cmd_bench_ingest(&args[2]);
        }
        "bench-tts" => {
            if args.len() < 3 {
                eprintln!("Uso: marginalia-cli bench-tts <document_id> [max_chunks]");
                process::exit(1);
            }
            let max_chunks = args.get(3).and_then(|s| s.parse().ok());
            cmd_bench_tts(&args[2], max_chunks);
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        other => {
            eprintln!("Comando sconosciuto: {other}");
            print_usage();
            process::exit(1);
        }
    }
}
