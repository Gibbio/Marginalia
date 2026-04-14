# marginalia-tts-mlx

Kokoro TTS via MLX Metal GPU. **macOS Apple Silicon only.**

Implements `SpeechSynthesizer` using `voice-tts` with `enable_compile()` for
fused Metal kernels. ~1000 ms for a 164-char chunk on M4 (~12× realtime).

Audio is written as 16-bit FLAC at 24 kHz (pure-Rust `flacenc`).

## Phonemizer

This crate ships its own minimal G2P pipeline in Rust (misaki-style):

1. `normalize_pauses` — brackets / dashes / smart quotes → commas
2. Split on clause punctuation (`. , ! ? : ; …` + CJK/Spanish variants)
3. `espeak-rs` phonemize per clause → IPA
4. Re-insert the punctuation between IPA pieces so Kokoro gets the prosody cues
5. `clean_ipa` — strip tie chars and combining diacritics

### Numbers

`.` and `,` between two ASCII digits are **not** treated as clause boundaries.
The full number ("2,5", "1.000.000", "3.14", "1,000,000") reaches espeak-ng
as a single token, which already knows the language-specific rules:

- IT: "2,5" → "due virgola cinque", "1.000" → "mille"
- EN: "2.5" → "two point five", "1,000" → "one thousand"

### Hard phoneme limit

Kokoro caps input at 512 tokens. Before calling `voice_tts::generate`, we
check the IPA length against 505 (leaves margin for BOS/EOS) and return an
error for over-long inputs instead of panicking inside the runtime Mutex.
Chunks are sized in core (`chunk_target_chars = 300`, `hard_max = 330`) to
stay under this limit at typical ~1.5× IPA expansion.

## Memory

MLX holds Metal buffer pools that can grow to several GB of unified memory
without explicit release. After each synthesize we:

1. `audio.eval()` — force execution before drop
2. `mlx_rs::transforms::compile::clear_cache()` — drop the JIT kernel cache
3. `mlx_clear_cache()` — drop the Metal buffer pool

The `mlx_clear_cache` symbol is declared via `extern "C"` (not yet exposed in
`mlx-rs`) and is linked through `mlx-sys`.

## Dependencies

- `Gibbio/voice-mlx` — forked `voice-tts` / `voice-nn` / `voice-dsp` with
  patched Kokoro decoder
- `oxideai/mlx-rs` — must use git HEAD; crates.io v0.25.3 bundles an older
  MLX C++ runtime and is noticeably slower
- `espeak-rs` — binding to espeak-ng for per-clause phonemization
- `flacenc` — pure-Rust FLAC encoder for output
