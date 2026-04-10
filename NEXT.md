# Next Steps

## In progress

- **Phonemizer tuning** — affinare la normalizzazione del testo per prosodia naturale in italiano (dialoghi, parentesi, punteggiatura). Riferimento: `hexgrad/misaki/misaki/espeak.py`.

## Short term

### Qualita' italiano
- [ ] Valutare fine-tune StyleTTS2 con dataset italiano (Mozilla Common Voice IT, ~100h gratis). Training: GPU 24GB+, 2-3 giorni. Produce ONNX integrabile al posto di Kokoro.
- [ ] Creare voice embeddings italiani migliori da campioni audio di speaker professionisti.
- [ ] Esplorare `espeak-rs` (binding Rust compilato) per eliminare la dipendenza da `espeak-ng` installato sul sistema.

### TTS cloud premium
- [ ] Integrare ElevenLabs e/o OpenAI TTS come backend opzionale a pagamento. API REST, implementare `SpeechSynthesizer` con un crate HTTP. L'utente sceglie locale (gratis, ~1s) o cloud (a pagamento, ~100ms, qualita' superiore specialmente per l'italiano).
- [ ] Configurazione nel toml:
  ```toml
  [tts]
  provider = "mlx"  # oppure "elevenlabs", "openai"

  [elevenlabs]
  api_key = "..."
  voice_id = "..."
  ```

### UX
- [ ] Playback automatico del chunk successivo alla fine del corrente (lettura continua senza premere /next).
- [ ] Indicatore visivo nella TUI durante la sintesi ("sintetizzando...").
- [ ] Barra di progresso nella TUI (chunk X/N).

## Medium term

### Multi-piattaforma
- [ ] Testare e ottimizzare Kokoro ONNX su Linux (CPU). Potrebbe servire XNNPACK o un backend diverso per ARM Linux.
- [ ] Valutare TTS per Windows (DirectML, CUDA).
- [ ] App desktop con Tauri (wrappa la TUI o una UI web).

### Modello
- [ ] Monitorare `mlx-rs` per nuove release su crates.io — quando includera' MLX C++ v0.31+, rimuovere la dipendenza da git e usare la versione stabile.
- [ ] Monitorare `voice-tts` / `voice-nn` per aggiornamenti — se l'autore torna su mlx-rs, allinearsi col suo repo invece di mantenere il fork `Gibbio/voice-mlx`.
- [ ] Valutare `compile_with_state` per JIT compilation del decoder quando mlx-rs lo supporta meglio. Potenziale -30% latenza.

### Import
- [ ] Supporto PDF (estrazione testo).
- [ ] Supporto EPUB.
- [ ] Import da URL (web scraping del contenuto).

## Long term

### Mobile
- [ ] iOS app con CoreML Kokoro nativo (modello FluidInference/kokoro-82m-coreml, benchmark 23x RTF su M4).
- [ ] Android app con ONNX Runtime (CPU o NNAPI).

### STT
- [ ] Riabilitare Vosk con soglia di confidenza piu' alta per evitare falsi positivi da rumore ambientale.
- [ ] Valutare Whisper locale per comandi vocali (piu' preciso di Vosk ma piu' lento).

### Sync e cloud
- [ ] Sincronizzazione posizione di lettura tra dispositivi.
- [ ] Backup documenti e note su cloud (opzionale).
