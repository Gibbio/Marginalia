# Contributing

## Setup

```bash
# Clone
git clone https://github.com/Gibbio/Marginalia.git
cd Marginalia

# Build and test
cargo build --release
cargo test

# Run TUI (auto-detects platform)
make tui-rs
```

### macOS Apple Silicon

MLX TTS requires Xcode with Metal Toolchain:

```bash
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -downloadComponent MetalToolchain
```

### Model assets

```bash
make bootstrap-beta    # download all models
make beta-doctor       # verify setup
```

## Structure

See `CLAUDE.md` for full architecture documentation.

## Conventions

- Rust, edition 2021
- `cargo fmt` and `cargo clippy` before committing
- Italian is the primary content language
- Don't add Python to the main codebase (benchmark/ is the exception)
- Keep TTS calls async in UI code
