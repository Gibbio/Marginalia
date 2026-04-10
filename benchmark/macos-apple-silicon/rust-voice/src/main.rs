use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

const PHRASES: &[(&str, &str)] = &[
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
];

const SAMPLE_RATE: u32 = 24000;

fn phonemize(text: &str) -> String {
    let mut child = Command::new("espeak-ng")
        .args(["-v", "it", "--ipa", "-q"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("espeak-ng not found");
    child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).trim().replace('\n', " ")
}

fn main() {
    eprintln!("  Loading Kokoro model (voice-tts, Candle+Metal)...");
    let t = Instant::now();
    let mut model = voice_tts::load_model("prince-canuma/Kokoro-82M")
        .expect("failed to load model");
    let load_ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("  Model loaded in {load_ms:.0}ms");

    let voice = voice_tts::load_voice("af_bella", None)
        .expect("failed to load voice");

    // Warmup
    eprintln!("  Warmup...");
    let ph = phonemize(PHRASES[0].1);
    let _ = voice_tts::generate(&mut model, &ph, &voice, 1.0);

    println!();
    println!("  {}", "=".repeat(75));
    println!("  Backend: voice-tts (Kokoro + Candle + Metal GPU) — Rust nativo");
    println!("  Model load: {load_ms:.0}ms");
    println!("  {}", "=".repeat(75));
    println!();
    println!(
        "  {:<8} {:>5} {:>8} {:>8} {:>9} {:>7} {:>8} {:>5}",
        "Label", "Chars", "Phonem", "Infer", "Total", "Audio", "Samples", "RTFx"
    );
    println!("  {}", "-".repeat(70));

    for &(label, text) in PHRASES {
        // Phonemize
        let t0 = Instant::now();
        let phonemes = phonemize(text);
        let phonemize_ms = t0.elapsed().as_secs_f64() * 1000.0;

        // Inference (Metal GPU)
        let t1 = Instant::now();
        let audio = voice_tts::generate(&mut model, &phonemes, &voice, 1.0)
            .expect("generate failed");
        let inference_ms = t1.elapsed().as_secs_f64() * 1000.0;

        let total_ms = phonemize_ms + inference_ms;
        let n_samples = audio.shape().iter().product::<i32>() as usize;
        let audio_duration_s = n_samples as f64 / SAMPLE_RATE as f64;
        let rtf = if inference_ms > 0.0 {
            audio_duration_s / (inference_ms / 1000.0)
        } else {
            0.0
        };

        println!(
            "  {:<8} {:>5} {:>7.0}ms {:>7.0}ms {:>8.0}ms {:>6.1}s {:>7} {:>5.1}x",
            label,
            text.len(),
            phonemize_ms,
            inference_ms,
            total_ms,
            audio_duration_s,
            n_samples,
            rtf,
        );
    }

    // Print key metric
    println!();
}
