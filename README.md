# Marginalia

Marginalia is a local AI-first voice reading and annotation engine.

## Quick Start

Run the TUI (auto-detects platform and enables GPU acceleration):

```bash
make tui-rs
```

On **macOS Apple Silicon** this auto-enables Kokoro TTS via MLX Metal GPU (~1s latency per chunk, 12x realtime). On other platforms it falls back to ONNX Runtime CPU or fake providers.

Run tests:

```bash
cargo test
```

## Architecture

One shared Rust engine with platform-specific host applications:

```
apps/
  tui-rs/              Terminal UI (desktop dev/admin tool)
  cli-rs/              CLI for testing core (ingest, read, bench)

crates/
  marginalia-core/             Domain, ports, application services (no external deps)
  marginalia-runtime/          Orchestration: core + storage + providers
  marginalia-storage-sqlite/   SQLite persistence
  marginalia-import-text/      Text/Markdown document importer
  marginalia-config/           Shared configuration types (voice commands, STT, TTS, playback)
  marginalia-models/           Model discovery, download, and cache management
  marginalia-tts-mlx/          Kokoro TTS via MLX Metal GPU (macOS Apple Silicon)
  marginalia-tts-kokoro/       Kokoro TTS via ONNX Runtime (cross-platform)
  marginalia-stt-apple/        Apple SFSpeechRecognizer — commands + dictation (macOS)
  marginalia-stt-whisper/      Whisper STT — commands + dictation (cross-platform)
  marginalia-stt-vosk/         Vosk STT (legacy, no longer wired in tui-rs)
  marginalia-playback-host/    Audio playback (rodio in-process)
  marginalia-provider-fake/    Fake providers for testing
  marginalia-devtools/         Development utilities

models/                Local model assets (downloaded via make bootstrap-beta)
benchmark/             TTS backend benchmark suite
```

## TTS Backends

| Backend | Platform | Latency (164ch) | RTFx | Requires |
|---|---|---|---|---|
| **marginalia-tts-mlx** | macOS Apple Silicon | ~1000ms | 12x | Xcode + Metal |
| marginalia-tts-kokoro | Cross-platform | ~5700ms | 2.3x | ONNX Runtime |

On macOS Apple Silicon, `make tui-rs` automatically selects the MLX backend. The Kokoro model downloads from HuggingFace on first use (~312MB).

See [`benchmark/README.md`](benchmark/README.md) for full benchmark results across 8 backends.

## Build Targets

```bash
# Default build (no native deps required)
cargo build --release

# TUI with platform auto-detection
make tui-rs

# TUI with explicit MLX TTS (macOS Apple Silicon only)
cargo build --release -p marginalia-tui --features mlx-tts

# CLI tool
cargo run -p marginalia-cli -- help

# Optional providers (need native libs)
cargo build -p marginalia-stt-apple     # needs Xcode (Swift helper)
cargo build -p marginalia-stt-whisper   # needs cmake + C++ compiler
cargo build -p marginalia-tts-mlx       # needs Xcode + Metal Toolchain
```

## Bootstrap

Download model assets:

```bash
# All Beta providers
make bootstrap-beta

# Individual
make bootstrap-kokoro    # Kokoro ONNX model + voices
make bootstrap-ort       # ONNX Runtime library
make bootstrap-whisper   # Whisper STT model (commands + dictation)
```

Apple STT (`marginalia-stt-apple`) needs no model download — it uses the
Neural Engine via SFSpeechRecognizer. Requires macOS Dictation to be enabled
(`System Settings → Keyboard → Dictation → ON`).


Verify setup:

```bash
make beta-doctor
```

## Build Your Own App

Marginalia is a **library-first** project. The TUI is just one host app — you
can build your own (GUI, mobile, web) using the runtime crates.

### 1. Add dependencies

```toml
[dependencies]
marginalia-runtime = { git = "https://github.com/Gibbio/Marginalia", tag = "v0.1.0-beta", features = ["host-playback"] }
marginalia-config  = { git = "https://github.com/Gibbio/Marginalia", tag = "v0.1.0-beta" }

# Optional — enable the providers you need:
# marginalia-runtime features: apple-stt, whisper-stt, mlx-tts, host-playback
```

### 2. Build the runtime (5 lines)

```rust
use marginalia_runtime::{RuntimeBuilder, RuntimeConfig};
use marginalia_config::*;

let output = RuntimeBuilder::new(".marginalia/app.sqlite3")
    .config(RuntimeConfig::default())
    .voice_commands(VoiceCommandsSection::default())
    .stt(SttSection::default())       // or configure engine = "apple"
    .mlx(MlxSection::default())       // or kokoro() for ONNX
    .playback(PlaybackSection::default())
    .build()?;

let runtime = output.runtime;
// output.stt_label, output.tts_label etc. for logging
```

The builder handles: database setup, provider wiring (TTS, STT, playback),
AEC echo cancellation, TTS cache, and session restore — all behind feature
flags. Your app does 5-10 lines instead of 500.

### 3. Execute commands and queries

The runtime exposes a JSON-based frontend API via `RuntimeFrontend`:

```rust
use marginalia_runtime::RuntimeFrontend;
use serde_json::json;

// Commands (mutate state)
let response = runtime.execute_frontend_command("start_session", json!({
    "target": "/path/to/document.txt"
}));

// Queries (read state)
let response = runtime.execute_frontend_query("get_session_snapshot", json!({}));
let session = &response.payload["session"];
println!("Reading: {} chunk {}", session["document_id"], session["chunk_index"]);
```

#### Available commands

| Command | Payload | Description |
|---|---|---|
| `ingest_document` | `{ "path": "file.txt" }` | Import a document into the library |
| `start_session` | `{ "target": "doc_id_or_path" }` | Start reading (auto-ingests if path) |
| `pause_session` | `{}` | Pause playback |
| `resume_session` | `{}` | Resume playback |
| `stop_session` | `{}` | Stop session (deactivates it) |
| `next_chunk` | `{}` | Advance to next chunk |
| `previous_chunk` | `{}` | Go back one chunk |
| `next_chapter` | `{}` | Skip to next chapter |
| `previous_chapter` | `{}` | Go to previous chapter |
| `repeat_chunk` | `{}` | Replay current chunk |
| `restart_chapter` | `{}` | Restart current chapter from first chunk |
| `create_note` | `{ "text": "..." }` | Attach a note to the current position |
| `restore_session` | `{}` | Restore last active session (auto-called by builder) |

#### Available queries

| Query | Payload | Returns |
|---|---|---|
| `get_app_snapshot` | `{}` | `{ "app": { "state", "document_count", ... } }` |
| `get_session_snapshot` | `{}` | `{ "session": { "document_id", "chunk_text", "playback_state", ... } }` |
| `get_document_view` | `{ "document_id": "..." }` | `{ "document": { "sections": [...], "chunks": [...] } }` |
| `list_documents` | `{}` | `{ "documents": [...] }` |
| `list_notes` | `{ "document_id": "..." }` | `{ "notes": [...] }` |
| `search_documents` | `{ "query": "..." }` | `{ "search": { "results": [...] } }` |
| `search_notes` | `{ "query": "..." }` | `{ "search": { "results": [...] } }` |
| `get_doctor_report` | `{}` | Provider health check report |
| `get_backend_capabilities` | `{}` | List of supported commands/queries |
| `auto_advance` | `{}` | Auto-advance if current chunk finished (returns `{ "advanced": bool }`) |

### 4. Subscribe to events

The runtime pushes typed events — no polling needed:

```rust
use marginalia_runtime::RuntimeEvent;

// Option A: channel (for polling loops / TUI)
let rx = runtime.subscribe_events();
match rx.try_recv() {
    Ok(RuntimeEvent::PlaybackFinished { document_id, chunk_index, .. }) => { ... }
    Ok(RuntimeEvent::ChunkAdvanced { .. }) => { ... }
    Ok(RuntimeEvent::SessionRestored { .. }) => { ... }
    _ => {}
}

// Option B: callback (for mobile / FFI)
runtime.on_event(Box::new(|event| {
    match event {
        RuntimeEvent::PlaybackFinished { .. } => { /* update UI */ }
        _ => {}
    }
}));
```

#### Event types

| Event | Fields | When |
|---|---|---|
| `PlaybackFinished` | `document_id, section_index, chunk_index` | A chunk finished playing naturally |
| `ChunkAdvanced` | `document_id, section_index, chunk_index` | Reading moved to a new chunk |
| `SessionRestored` | `session_id, document_id, section_index, chunk_index` | Session restored from database on startup |
| `SessionStopped` | `document_id` | Session explicitly stopped |
| `Error` | `message` | Runtime error the app should surface |

### 5. Configure voice commands

```rust
use marginalia_config::VoiceCommandsSection;

let mut vc = VoiceCommandsSection::default();
vc.pause = vec!["pause".into(), "hold".into()];
vc.next = vec!["next".into(), "forward".into()];

// Resolve a recognized phrase to an action:
match vc.resolve_action("go forward please") {
    Some("next") => { /* advance */ }
    Some("pause") => { /* pause */ }
    _ => { /* not a command */ }
}
```

### 6. Download models programmatically

```rust
use marginalia_models::ModelManager;

let models = ModelManager::new()?;

// Whisper STT model (~460MB, cached after first download)
let whisper_path = models.ensure_whisper("ggml-small.bin")?;

// Kokoro voice embedding
let voice_path = models.ensure_kokoro_voice("if_sara")?;
```

### Minimal example: ingest + read

```rust
use marginalia_runtime::{RuntimeBuilder, RuntimeConfig};
use marginalia_config::*;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = RuntimeBuilder::new(".marginalia/demo.sqlite3")
        .config(RuntimeConfig::default())
        .build()?;
    let mut runtime = output.runtime;

    // Import a document
    runtime.execute_frontend_command("ingest_document", json!({
        "path": "my-book.txt"
    }));

    // Start reading (auto-plays first chunk with TTS)
    runtime.execute_frontend_command("start_session", json!({
        "target": "my-book.txt"
    }));

    // The runtime handles: TTS synthesis, playback, auto-advance to next
    // chunk, session persistence. Your app just needs to render the UI.

    Ok(())
}
```
