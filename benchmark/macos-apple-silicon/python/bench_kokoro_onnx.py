#!/usr/bin/env python3
"""Benchmark Kokoro TTS via kokoro-onnx (Python ONNX Runtime) on Apple Silicon."""

import sys
import time

PHRASES = [
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
]

SAMPLE_RATE = 24000


def run_benchmark():
    try:
        from kokoro_onnx import Kokoro
    except ImportError:
        print("  SKIP: kokoro-onnx not installed (pip install kokoro-onnx)")
        sys.exit(1)

    print("\n  Loading Kokoro model (kokoro-onnx)...")
    t0 = time.time()
    kokoro = Kokoro("/tmp/kokoro-v1.0.onnx", "/tmp/voices-v1.0.bin")
    load_ms = (time.time() - t0) * 1000
    print(f"  Model loaded in {load_ms:.0f}ms\n")

    print(f"  {'='*60}")
    print(f"  Backend: kokoro-onnx (Python ONNX Runtime CPU)")
    print(f"  Model load: {load_ms:.0f}ms")
    print(f"  {'='*60}\n")

    # Warmup
    print("  Warmup...", end="", flush=True)
    kokoro.create("Test.", voice="af_bella", speed=1.0, lang="it")
    print(" done")

    print(f"\n  {'Label':<8} {'Chars':>5} {'Total':>8} {'Audio':>6} {'RTFx':>6}")
    print(f"  {'-'*40}")

    results = []
    for label, text in PHRASES:
        t = time.time()
        samples, sr = kokoro.create(text, voice="af_bella", speed=1.0, lang="it")
        elapsed_ms = (time.time() - t) * 1000

        audio_duration_s = len(samples) / sr
        rtf = audio_duration_s / (elapsed_ms / 1000) if elapsed_ms > 0 else 0

        print(f"  {label:<8} {len(text):>5} {elapsed_ms:>7.0f}ms {audio_duration_s:>5.1f}s {rtf:>5.1f}x")
        results.append({
            "label": label,
            "chars": len(text),
            "total_ms": elapsed_ms,
            "audio_duration_s": audio_duration_s,
            "rtf": rtf,
        })

    med = next((r for r in results if r["label"] == "medium"), None)
    if med:
        print(f"\n  Key metric (medium phrase, {med['chars']} chars):")
        print(f"    Total:  {med['total_ms']:.0f}ms")
        print(f"    Audio:  {med['audio_duration_s']:.1f}s")
        print(f"    RTF:    {med['rtf']:.1f}x realtime")


if __name__ == "__main__":
    run_benchmark()
