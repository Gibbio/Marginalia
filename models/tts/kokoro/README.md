# Kokoro TTS Assets

This directory holds the Kokoro model assets used by the Beta runtime.

## Expected layout

```
.kokoro-assets/          (or wherever MARGINALIA_KOKORO_ASSETS points)
├── kokoro.onnx          (or model.onnx / kokoro-v1.0.onnx)
├── config.json
├── voices/
│   ├── af.bin           (default voice)
│   └── <other>.bin
└── lib/
    ├── libonnxruntime.so      (Linux)
    └── libonnxruntime.dylib   (macOS)
```

The runtime searches these filenames in order. Place assets in a
directory of your choice and point `MARGINALIA_KOKORO_ASSETS` at it.

## Downloading the model and voices

The Kokoro-82M ONNX model and config are published on HuggingFace
at `hexgrad/Kokoro-82M`. Download with the HuggingFace CLI:

```bash
pip install huggingface_hub
huggingface-cli download hexgrad/Kokoro-82M \
    kokoro.onnx config.json \
    --local-dir .kokoro-assets
```

Voice embeddings live in the `voices/` subdirectory of the same repo.
Download the voices you need (default is `af`):

```bash
huggingface-cli download hexgrad/Kokoro-82M \
    voices/af.bin \
    --local-dir .kokoro-assets
```

## Downloading ONNX Runtime

The runtime library must match the version of the `ort` crate in use.
Check `Cargo.lock` for the `ort` version, then download the
corresponding release from the ONNX Runtime GitHub releases page
(`microsoft/onnxruntime`).

Place the shared library under `lib/` inside your assets directory:

```bash
mkdir -p .kokoro-assets/lib
# Linux example
cp /path/to/libonnxruntime.so.1.x.y .kokoro-assets/lib/libonnxruntime.so
# macOS example
cp /path/to/libonnxruntime.1.x.y.dylib .kokoro-assets/lib/libonnxruntime.dylib
```

Alternatively, set `ORT_DYLIB_PATH` to the full path of the library
and skip placing it under `lib/`.

## Verifying the setup

```bash
make beta-doctor
# or directly:
cargo run -p marginalia-devtools -- kokoro-doctor --assets-root .kokoro-assets
```

A fully ready setup reports no missing assets and a successful ONNX
session probe.
