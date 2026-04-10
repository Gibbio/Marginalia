#!/usr/bin/env bash
# TTS Backend Benchmark — macOS Apple Silicon
# Usage: ./run.sh <kokoro_assets_dir>
#
# Runs all available backends and prints comparison.

set -euo pipefail

ASSETS="${1:-../../models/tts/kokoro}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  TTS Backend Benchmark — macOS Apple Silicon             ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo ""
echo "  Assets: $ASSETS"
echo "  Date:   $(date)"
echo "  Chip:   $(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo unknown)"
echo "  Cores:  $(sysctl -n hw.ncpu)"
echo "  RAM:    $(( $(sysctl -n hw.memsize) / 1024 / 1024 / 1024 )) GB"
echo ""

# ── 1. ONNX Runtime (Rust) ──────────────────────────────────────
echo "━━━ [1/3] ONNX Runtime (Rust, CPU) ━━━"
if command -v cargo >/dev/null 2>&1; then
    cd "$SCRIPT_DIR/rust"
    cargo build --release --quiet 2>/dev/null
    ./target/release/tts-bench ort "$ASSETS"
    echo ""
    echo "━━━ [1b/3] ORT Thread Sweep ━━━"
    ./target/release/tts-bench ort-sweep "$ASSETS"
    cd "$SCRIPT_DIR"
else
    echo "  SKIP: cargo not found"
fi

# ── 2. MLX (Python) ─────────────────────────────────────────────
echo ""
echo "━━━ [2/3] MLX Audio (Python, Apple GPU) ━━━"
if python3 -c "import mlx_audio" 2>/dev/null; then
    python3 "$SCRIPT_DIR/python/bench_mlx.py"
else
    echo "  SKIP: mlx-audio not installed (pip install mlx-audio)"
fi

# ── 3. PyTorch MPS (Python) ─────────────────────────────────────
echo ""
echo "━━━ [3/3] PyTorch + MPS (Python, Metal GPU) ━━━"
if python3 -c "import torch; import kokoro" 2>/dev/null; then
    python3 "$SCRIPT_DIR/python/bench_pytorch_mps.py"
else
    echo "  SKIP: torch/kokoro not installed (pip install torch kokoro)"
fi

echo ""
echo "Done."
