# marginalia-tts-kokoro

Kokoro TTS asset discovery and Beta runtime integration scaffolding.

This crate does not yet perform ONNX inference.

Current responsibilities:

- resolve Kokoro asset locations inside `models/tts/kokoro`
- validate whether the expected model files are present
- expose a readiness report that desktop hosts and devtools can use during the
  Python removal migration
