# Marginalia TUI

Terminal UI for Marginalia, built with ratatui.

## Run

```bash
make tui-rs
```

On macOS Apple Silicon this auto-enables:
- Kokoro MLX (Metal GPU, ~1s per chunk)
- Apple STT (SFSpeechRecognizer, commands + dictation)

On other platforms it falls back to Kokoro ONNX and Whisper STT.

Already-synthesized chunks are cached — revisiting a chunk is instant.
Text is phonemized clause-by-clause via espeak-ng, following the
[misaki](https://github.com/hexgrad/misaki) reference G2P for Kokoro.

## Configuration

Edit `apps/tui-rs/marginalia.toml`. The file is regenerated from
`marginalia.toml.template` by `make tui-rs` — edits are preserved unless the
template changes.

```toml
database_path = ".marginalia/beta.sqlite3"

# ── TTS — Kokoro via MLX (macOS Apple Silicon) ───────────────
[mlx]
voice = "if_sara"                       # Italian female

# Available voices:
#   Italian:  if_sara (F), im_nicola (M)
#   English:  af_bella, af_heart, af_sarah, af_sky (F)
#             am_adam, am_michael (M)
#   British:  bf_emma, bf_alice (F), bm_george, bm_daniel (M)
#   Other:    50+ voices auto-download from HuggingFace on first use

# ── TTS — Kokoro ONNX (cross-platform fallback) ──────────────
# [kokoro]
# assets_root = "models/tts/kokoro"     # needs make bootstrap-kokoro
# phonemizer_program = "espeak-ng"
# phonemizer_args = ["-v", "it", "--ipa", "-q"]

# ══════════════════════════════════════════════════════════════
#  VOICE COMMANDS — words mapped to actions
# ══════════════════════════════════════════════════════════════
[voice_commands]
pause        = ["pausa", "ferma"]
next         = ["avanti", "prossimo"]
back         = ["indietro"]
stop         = ["stop", "basta"]
repeat       = ["ripeti"]
resume       = ["riprendi", "continua"]
next_chapter = ["prossimo capitolo", "capitolo avanti"]
prev_chapter = ["capitolo indietro", "capitolo precedente"]
bookmark     = ["segna", "segnalibro"]
note         = ["nota", "appunto"]
where        = ["dove sono", "posizione"]

# ══════════════════════════════════════════════════════════════
#  STT — speech-to-text engine (commands + dictation)
# ══════════════════════════════════════════════════════════════
[stt]
engine   = "apple"      # "apple" (macOS Neural Engine) or "whisper"
language = "it"         # ISO ("it") or BCP-47 ("it-IT"); auto-converted
debug    = true         # show raw transcript in the Log pane

# Apple-engine settings (used when engine = "apple").
# Requires System Settings → Keyboard → Dictation → ON.
[stt.apple]

# Whisper-engine settings (used when engine = "whisper").
# [stt.whisper]
# model_path = "models/stt/whisper/ggml-small.bin"

# Tuning profile for SHORT utterances (voice commands).
# Defaults: silence_timeout = 0.8, max_record_seconds = 4, speech_threshold = 500
[stt.commands]
# silence_timeout    = 0.8
# max_record_seconds = 4
# speech_threshold   = 500

# Tuning profile for LONG utterances (note dictation via /note).
# Defaults: silence_timeout = 1.5, max_record_seconds = 60, speech_threshold = 500
[stt.dictation]
# silence_timeout    = 1.5
# max_record_seconds = 60
# speech_threshold   = 500

# ── Playback ──────────────────────────────────────────────────
[playback]
# fake = true   # headless / CI environments
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
| `/note <text>` | Add a note (text) |
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

## Voice commands and dictation

Voice commands are mapped from trigger words to actions in `[voice_commands]`.
The STT backend listens continuously; when a spoken phrase matches one of the
trigger words, the corresponding action executes. Add synonyms in any language
without touching code.

| Action | What it does | Default triggers |
|---|---|---|
| **pause** | Pause playback | pausa, ferma |
| **next** | Next chunk | avanti, prossimo |
| **back** | Previous chunk | indietro |
| **stop** | Stop session | stop, basta |
| **repeat** | Repeat current chunk | ripeti |
| **resume** | Resume playback | riprendi, continua |
| **next_chapter** | Skip to next chapter | prossimo capitolo |
| **prev_chapter** | Previous chapter | capitolo indietro |
| **bookmark** | Save position as note | segna, segnalibro |
| **note** | Start note dictation | nota, appunto |
| **where** | Show current position in Log | dove sono, posizione |

To add a new trigger word, edit the toml. To add a new action, also add
a match arm in `app.rs` → `handle_voice_command`.

Enable `[stt] debug = true` to see what the mic hears in the Log pane.

### STT engines

A **single** engine handles BOTH voice commands and note dictation, with
per-context tuning in `[stt.commands]` and `[stt.dictation]`.

| Engine | Platform | Latency (commands) | Dictation | Requires |
|---|---|---|---|---|
| **Apple** | macOS | ~0.2-0.3s | Yes (same process, mode-switch) | macOS Dictation enabled |
| **Whisper** | Cross-platform | ~2s | Yes (separate profile) | Whisper ggml model (~460MB) |

**Apple** (recommended on macOS): uses `SFSpeechRecognizer` on the Neural Engine
via a Swift helper subprocess. Zero models to download, fastest response.
A single helper process handles both commands and dictation — it switches
mode on demand, so there is exactly one mic stream open per session.

- **Start of note**: a system Tink sound plays the moment dictation begins
- **End of note**: a system Pop sound plays when dictation is committed
- **Privacy requirement**: `System Settings → Keyboard → Dictation → ON`

**Whisper**: builds two `WhisperConfig` profiles from the same model — one
tuned for short commands (`[stt.commands]`, default 4s max / 0.8s silence)
and one for dictation (`[stt.dictation]`, default 60s max / 1.5s silence).

Download the Whisper model with `make bootstrap-whisper`.

### Dictation (note recording)

Triggered by the voice command **nota** or by a future `/note` keystroke.
The STT engine switches from command mode to dictation mode, records until
silence (or `max_record_seconds`), and returns the transcribed text. The
mode automatically switches back to command recognition after each note.

Dictation tuning is fully independent from command tuning — setting a short
`silence_timeout` in `[stt.commands]` will NOT truncate dictated notes.

### Echo filter

While the TTS is reading a chunk aloud, the microphone inevitably picks up
the playback. Without protection, trigger words that happen to appear in
the document (e.g. the Italian word *"avanti"* = "next") would fire
spurious commands as soon as the TTS pronounces them.

The TUI wires the external crate
[`stt-echo-filter`](https://github.com/Gibbio/stt-echo-filter) into
`handle_voice_command` to do a **post-STT, word-level** echo rejection:
every STT utterance is compared against the text of the currently-playing
chunk, and any word already present in that chunk is "consumed" by a
per-word budget before trigger matching. What's left is what the user
actually said. If the filter absorbs everything, the utterance is dropped
with an `[echo] dropped: ...` line in the Log pane (when `stt.debug = true`).

This is a triage filter, not a true acoustic echo canceller. It has one
known trade-off: if the user legitimately speaks a word that is also in
the current chunk, the budget treats it as echo and the command is
ignored. Use a synonym in `[voice_commands]` to work around it
(e.g. `next = ["avanti", "prossimo"]`).

## Build features

| Feature | What it enables |
|---|---|
| `mlx-tts` | Kokoro via MLX Metal GPU (macOS Apple Silicon) |
| `apple-stt` | Apple SFSpeechRecognizer (commands + dictation, macOS) |
| `whisper-stt` | Whisper STT (commands + dictation, cross-platform) |
| `whisper-stt-metal` | Whisper with Metal acceleration (macOS) |
| `whisper-stt-cuda` | Whisper with CUDA acceleration (Linux/Windows) |
| `kokoro-coreml` | _(broken — CoreML incompatible with Kokoro)_ |
| `kokoro-cuda` | Kokoro ONNX via CUDA EP |

`make tui-rs` auto-selects features based on platform and available models.
