# marginalia-tts-kokoro

Kokoro TTS asset discovery and Beta runtime integration scaffolding.

Current responsibilities:

- resolve Kokoro asset locations inside `models/tts/kokoro`
- validate whether the expected model and voice assets are present
- load `config.json` vocabulary and map phoneme symbols to token IDs
- resolve an ONNX Runtime dynamic library for local probing
- expose a doctor report that desktop hosts and devtools can use during the
  Python removal migration
- attempt to open the Kokoro ONNX model and report whether a session can be
  created
- run low-level ONNX inference from either phoneme strings or precomputed token
  IDs, plus voice bin and speed

Expected runtime layout:

- Kokoro model assets under `models/tts/kokoro`
- `config.json` with the official `vocab` mapping from symbol to token ID
- voice bins under `models/tts/kokoro/voices/<voice>.bin` such as `voices/af.bin`
- ONNX Runtime dynamic library either:
  - provided via `ORT_DYLIB_PATH`, or
  - placed under `models/tts/kokoro/lib/` or a similar nested directory

This crate still does not implement grapheme-to-phoneme. The current ONNX path
starts from phoneme strings and uses the official `config.json` vocabulary to
produce token IDs.
