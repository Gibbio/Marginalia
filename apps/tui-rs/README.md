# Marginalia TUI

Terminal UI for Marginalia, built with ratatui.

## Run

```bash
make tui-rs
```

On macOS Apple Silicon this auto-enables Kokoro MLX (Metal GPU, ~1s per chunk).
On other platforms it falls back to Kokoro ONNX or fake TTS.

Already-synthesized chunks are cached — revisiting a chunk is instant.
Text is phonemized clause-by-clause via espeak-ng, following the
[misaki](https://github.com/hexgrad/misaki) reference G2P for Kokoro.

## Configuration

Edit `apps/tui-rs/marginalia.toml`:

```toml
database_path = ".marginalia/beta.sqlite3"

# ── TTS — MLX (macOS Apple Silicon, auto-enabled) ────────────
[mlx]
# model = "prince-canuma/Kokoro-82M"   # HuggingFace repo (auto-download)
voice = "if_sara"                       # Italian female voice

# Available voices:
#   Italian:  if_sara (F), im_nicola (M)
#   English:  af_bella, af_heart, af_sarah, af_sky (F)
#             am_adam, am_michael (M)
#   British:  bf_emma, bf_alice (F), bm_george, bm_daniel (M)
#   Other:    50+ voices auto-download from HuggingFace on first use

# ── TTS — Kokoro ONNX (cross-platform fallback) ──────────────
[kokoro]
assets_root = "models/tts/kokoro"       # needs make bootstrap-kokoro
# tts_cache_dir = ".marginalia/tts-cache"
phonemizer_program = "espeak-ng"
phonemizer_args = ["-v", "it", "--ipa", "-q"]

# ── STT — Vosk (voice commands) ──────────────────────────────
[vosk]
model_path = "models/stt/vosk/vosk-model-small-it-0.22"
commands = ["pausa", "avanti", "indietro", "stop"]

# ── STT — Whisper (note dictation) ───────────────────────────
[whisper]
model_path = "models/stt/whisper/ggml-base.bin"
language = "it"

# ── Playback ──────────────────────────────────────────────────
[playback]
# fake = true   # headless/CI environments
```

## Commands

| Command | Description |
|---|---|
| `/ingest <file>` | Import a .txt or .md file |
| `/play <path\|id>` | Start reading session |
| `/pause` | Pause playback |
| `/resume` | Resume playback |
| `/stop` | Stop session |
| `/next` | Next chapter |
| `/back` | Previous chunk |
| `/repeat` | Repeat current chunk |
| `/restart` | Restart current chapter |
| `/note <text>` | Add a voice note |
| `/help` | Show available commands |

## Keyboard shortcuts

With an empty command bar:

| Key | Action |
|---|---|
| `Up` / `Down` | Previous / next chunk |
| `Left` / `Right` | Previous / next chapter |
| `PageUp` / `PageDown` | Scroll document pane |
| `Tab` | Autocomplete command |
| `Ctrl-P` / `Ctrl-N` | Command history |
| `Ctrl-C` (x2) | Quit |

## Build features

| Feature | What it enables |
|---|---|
| `mlx-tts` | Kokoro via MLX Metal GPU (macOS Apple Silicon) |
| `vosk-stt` | Vosk voice commands (needs libvosk) |
| `whisper-stt` | Whisper dictation (needs whisper.cpp) |
| `whisper-stt-metal` | Whisper with Metal acceleration |
| `kokoro-coreml` | _(broken — CoreML incompatible with Kokoro)_ |

`make tui-rs` auto-selects features based on platform and available models.
