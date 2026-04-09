# Kokoro TTS Assets

This directory is reserved for Kokoro model assets, manifests, and packaging
metadata used by the Beta runtime.

Expected Beta layout:

- `kokoro.onnx` or another configured model filename at the root
- `config.json` with the official Kokoro vocabulary
- `voices/<voice>.bin` voice embeddings, for example `voices/af.bin`
- optional ONNX Runtime dynamic library under `lib/` or via `ORT_DYLIB_PATH`
