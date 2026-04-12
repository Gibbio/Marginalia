# TTS Backend Benchmark

Confronto sistematico dei backend TTS per Marginalia, focalizzato su latenza e throughput per uso interattivo (skip tra chunk/capitoli).

## Risultati — Apple Silicon M4

Frasi di test in italiano, phonemizzate con `espeak-ng`. Il modello e' Kokoro-82M (eccetto Piper che usa modelli VITS dedicati).

| Backend | Tiny (25ch) | Medium (164ch) | Long (315ch) | RTFx | Rust | GPU |
|---|---|---|---|---|---|---|
| **Piper riccardo** (x_low, 27MB) | 49ms | 201ms | 353ms | 47.9x | si | no |
| **Piper paola** (medium, 61MB) | 62ms | 260ms | 442ms | 29.9x | si | no |
| **kokoro-mlx-rs** (mlx-rs git) | 226ms | **1042ms** | 2545ms | **12.0x** | si | si |
| Python MLX (mlx-audio) | 271ms | 1118ms | 2056ms | 11.1x | no | si |
| voice-tts 0.2 (mlx-rs crates.io) | 287ms | 1916ms | 2852ms | 6.5x | si | si |
| Candle 0.10 + Metal | 1160ms | 5835ms | 10601ms | 2.1x | si | si |
| Python ONNX (kokoro-onnx) | 1247ms | 3077ms | 4965ms | 3.8x | no | no |
| Rust ONNX RT 4 thread | 1263ms | 4981ms | 9278ms | 2.7x | si | no |
| Rust ONNX RT auto | 1380ms | 5675ms | 9771ms | 2.3x | si | no |

**RTFx** = realtime factor (audio_duration / inference_time). Piu' alto = piu' veloce.

## Cosa significano i risultati

**Per uso interattivo** (skip tra chunk), la latenza sul primo chunk e' critica:

- **< 100ms**: istantaneo — **Piper** e' l'unico in questa fascia
- **200-300ms**: fluido — **kokoro-mlx-rs** e **Python MLX**
- **1-2s**: percepibile ma usabile con pre-sintesi in pipeline
- **5-10s**: inutilizzabile per skip interattivo

**Per qualita' audio**, Kokoro e' nettamente superiore a Piper (voce piu' naturale e espressiva).

## Analisi dei backend

### Piper (CPU, ONNX)
Modelli VITS piccoli (27-61MB), ottimizzati per CPU. Velocissimo ma voce piu' sintetica. Solo una voce italiana disponibile (riccardo x_low, paola medium). Integrabile in Rust via ONNX Runtime direttamente.

### kokoro-mlx-rs (MLX, Metal GPU) ★
Il nostro crate: wrappa `voice-tts` 0.2 (mlx-rs) con `enable_compile()` per kernel fusion. **Raggiunge le performance di Python MLX**, tutto in Rust nativo. Richiede `mlx-rs` da git (non crates.io) per avere MLX C++ aggiornato.

- Repo: https://github.com/Gibbio/kokoro-mlx-rs
- Solo macOS Apple Silicon

### Python MLX (mlx-audio)
Il riferimento. Usa `mx.compile(decoder)` per JIT del decoder. Richiede Python runtime.

### voice-tts 0.2 (mlx-rs, crates.io)
Stesso codice di kokoro-mlx-rs ma con mlx-rs 0.25.3 da crates.io che bundla MLX C++ v0.25.1 (vecchio). **Il 46% di gap con Python MLX era interamente dovuto alla versione MLX C++ outdated.**

### Candle 0.10 + Metal
La versione attuale di `voice` (git HEAD). Usa Candle di HuggingFace con Metal backend. **5.6x piu' lento di mlx-rs** perche' Candle dispatcha ogni operazione come kernel Metal separato, senza kernel fusion. Il feature `accelerate` (Apple BLAS) non aiuta — il bottleneck e' il dispatch overhead, non il compute.

L'autore di `voice` e' passato da mlx-rs a Candle per supporto Whisper STT (`candle-transformers`) e cross-platform (CUDA). Nessuno ha segnalato la regressione di performance.

### ONNX Runtime (CPU)
Il backend attuale di Marginalia (`marginalia-tts-kokoro`). Lento perche' gira interamente su CPU senza accelerazione GPU. La versione Python (`kokoro-onnx`) e' piu' veloce della versione Rust probabilmente per un modello diverso (v1.0 fp32 vs vecchio quantizzato).

## Problemi trovati durante il benchmark

### ORT 1.20 → 1.24 (bloccante)
Il crate `ort` v2.0.0-rc.12 con feature `api-24` richiedeva ONNX Runtime 1.24+. Avevamo la 1.20.1 che causava un deadlock in `init_from()`. Fix: aggiornato la dylib a ORT 1.24.4, aggiornato `ORT_VERSION` nel Makefile.

### mlx-rs crates.io vs git (performance)
`mlx-rs` 0.25.3 su crates.io bundla MLX C++ v0.25.1. Il git HEAD bundla ~v0.31. La differenza: **1916ms vs 1042ms** sullo stesso codice. Solo l'aggiornamento della dipendenza C++ da' un +85% di throughput.

### CoreML EP con Kokoro (non funziona)
ONNX Runtime CoreML Execution Provider non supporta il modello Kokoro per via delle shape dinamiche nel grafo. Testato con `NeuralNetwork`, `MLProgram` e `with_static_input_shapes` — nessuno funziona. Il modello dovrebbe essere convertito con shape bounds espliciti tramite `coremltools`, ma richiede Python < 3.14.

## Struttura benchmark

```
benchmark/
├── README.md                           (questo file)
└── macos-apple-silicon/
    ├── run.sh                          Script per lanciare tutti i benchmark
    ├── rust/                           ONNX Runtime (Rust)
    │   ├── Cargo.toml
    │   └── src/main.rs                 tts-bench: ort, ort-sweep, piper
    ├── rust-voice/                     voice-tts benchmark (mlx-rs da crates.io)
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── rust-mlx/                       mlx-rs probe (verifica GPU Metal)
    │   ├── Cargo.toml
    │   └── src/main.rs
    ├── kokoro-mlx-rs/                  ★ Il nostro crate ottimizzato
    │   ├── Cargo.toml                  (mlx-rs da git + enable_compile)
    │   ├── src/lib.rs
    │   └── examples/{bench,generate}.rs
    ├── voice-fork/                     Candle 0.10 benchmark
    │   └── crates/voice-tts/examples/bench.rs
    ├── voice-patch/                    voice-nn/voice-tts con patch locali
    │   ├── voice-nn/                   (random ops rimossi dal decoder)
    │   └── voice-tts/
    └── python/
        ├── bench_mlx.py                MLX Audio (Python)
        ├── bench_kokoro_onnx.py        kokoro-onnx (Python ONNX)
        ├── bench_melotts.py            MeloTTS (non installabile su Py 3.14)
        ├── bench_pytorch_mps.py        PyTorch+MPS (non installabile su Py 3.14)
        └── requirements.txt
```

## Come riprodurre

```bash
# ONNX Runtime (Rust)
cd rust && cargo build --release
./target/release/tts-bench ort ../../models/tts/kokoro
./target/release/tts-bench ort-sweep ../../models/tts/kokoro
./target/release/tts-bench piper \
  ../../models/tts/piper/it_IT-paola-medium.onnx \
  ../../models/tts/piper/it_IT-paola-medium.onnx.json \
  ../../models/tts/kokoro/lib/libonnxruntime.dylib

# kokoro-mlx-rs (Rust, Metal GPU) — richiede Xcode con Metal Toolchain
cd kokoro-mlx-rs && cargo run --release --example bench

# Python MLX
python3 -m venv .venv && source .venv/bin/activate
pip install mlx-audio misaki num2words
python3 python/bench_mlx.py
```
