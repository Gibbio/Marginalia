# marginalia-tts-kokoro

Kokoro TTS asset discovery and Beta runtime integration scaffolding.

Current responsibilities:

- resolve Kokoro asset locations inside `models/tts/kokoro`
- validate whether the expected model files are present
- resolve an ONNX Runtime dynamic library for local probing
- expose a doctor report that desktop hosts and devtools can use during the
  Python removal migration
- attempt to open the Kokoro ONNX model and report whether a session can be
  created

Expected runtime layout:

- Kokoro model assets under `models/tts/kokoro`
- ONNX Runtime dynamic library either:
  - provided via `ORT_DYLIB_PATH`, or
  - placed under `models/tts/kokoro/lib/` or a similar nested directory
