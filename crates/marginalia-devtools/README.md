# marginalia-devtools

Development tooling for the Marginalia Beta engine.

The first tool is a small Rust CLI for exercising the runtime without the
Python backend or the TUI.

Current commands:

- `fake-play <document>`
- `kokoro-doctor [assets_root]`
  - checks Kokoro assets
  - resolves the ONNX Runtime dynamic library
  - attempts to open the ONNX model
- `kokoro-synthesize-text [assets_root] <output_dir> <text>`
  - exercises the Rust `SpeechSynthesizer` wrapper
  - currently expects text prefixed with `phon:` or `ipa:`
- `kokoro-encode-phonemes [assets_root] <phoneme_text>`
  - loads `config.json`
  - maps phoneme symbols to Kokoro token IDs
- `kokoro-run-phonemes [assets_root] <voice> <output_wav> <phoneme_text> [speed]`
  - tokenizes phoneme text with `config.json`
  - runs low-level Kokoro ONNX inference
  - writes the generated waveform to a local WAV file
- `kokoro-run-tokens [assets_root] <voice> <output_wav> <token_ids_csv> [speed]`
  - runs low-level Kokoro ONNX inference from token IDs
  - writes the generated waveform to a local WAV file
- `sqlite-ingest <db> <document>`
- `sqlite-list-documents <db>`
- `sqlite-play <db> <document>`
- `sqlite-play-target <db> <path|document_id>`
- `sqlite-pause <db>`
- `sqlite-resume <db>`
- `sqlite-stop <db>`
- `sqlite-repeat <db>`
- `sqlite-next-chunk <db>`
- `sqlite-previous-chunk <db>`
- `sqlite-next-chapter <db>`
- `sqlite-previous-chapter <db>`
- `sqlite-restart-chapter <db>`
- `sqlite-note <db> <text>`
- `sqlite-status <db>`
