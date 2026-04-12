# Kokoro MLX Assets

This directory holds the Kokoro MLX model weights and voice embeddings
used by `marginalia-tts-mlx` on macOS Apple Silicon.

## Expected layout

```
models/tts/mlx/
├── kokoro-v1_0.safetensors   # model weights (~330 MB)
└── voices/
    ├── if_sara.safetensors   # Italian female voice
    └── im_nicola.safetensors # Italian male voice
```

## Downloading

Run the bootstrap target (requires internet access, one-time):

```bash
make bootstrap-mlx
```

This downloads `kokoro-v1_0.safetensors` and the configured voices from
`prince-canuma/Kokoro-82M` on HuggingFace, using `hf`/`huggingface-cli`
if available, otherwise `curl`.

`make tui-rs` calls `bootstrap-mlx` automatically before building, so
on a fresh clone the download happens as part of the first build.

## Adding voices

Override `MLX_VOICES` to download additional voices:

```bash
make bootstrap-mlx MLX_VOICES="if_sara im_nicola af_bella am_adam"
```

Available voices: see `prince-canuma/Kokoro-82M` on HuggingFace.
Then set `voice = "<name>"` in the `[mlx]` section of `marginalia.toml`.
