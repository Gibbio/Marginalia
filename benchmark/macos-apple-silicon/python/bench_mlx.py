#!/usr/bin/env python3
"""Benchmark Kokoro TTS via mlx-audio on Apple Silicon (MLX GPU)."""

import sys
import time
import types

# Stub spacy (not needed for Italian via espeak, and doesn't build on Python 3.14)
if "spacy" not in sys.modules:
    spacy = types.ModuleType("spacy")
    spacy.load = lambda *a, **kw: None
    sys.modules["spacy"] = spacy

PHRASES = [
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
]

MODEL_ID = "prince-canuma/Kokoro-82M"
SAMPLE_RATE = 24000


def run_benchmark():
    try:
        from mlx_audio.tts import load_model
    except ImportError:
        print("  SKIP: mlx-audio not installed (pip install mlx-audio misaki)")
        sys.exit(1)

    print("\n  Loading Kokoro model via mlx-audio...")
    t0 = time.time()
    model = load_model(MODEL_ID)
    load_ms = (time.time() - t0) * 1000
    print(f"  Model loaded in {load_ms:.0f}ms")

    print(f"\n  {'='*60}")
    print(f"  Backend: MLX Audio (Apple Silicon GPU)")
    print(f"  Model load: {load_ms:.0f}ms")
    print(f"  {'='*60}")

    # Warmup
    print("\n  Warmup...", end="", flush=True)
    for chunk in model.generate(text="Test.", voice="af_bella", speed=1.0, lang_code="i"):
        pass
    print(" done")

    print(f"\n  {'Label':<8} {'Chars':>5} {'Total':>8} {'Audio':>6} {'RTFx':>6}")
    print(f"  {'-'*40}")

    results = []
    for label, text in PHRASES:
        t = time.time()
        total_audio_samples = 0
        for chunk in model.generate(text=text, voice="af_bella", speed=1.0, lang_code="i"):
            audio = getattr(chunk, "audio", None)
            if audio is not None and hasattr(audio, "shape"):
                total_audio_samples += audio.shape[-1]
        audio_duration_s = total_audio_samples / SAMPLE_RATE
        elapsed_ms = (time.time() - t) * 1000
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
