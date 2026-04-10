#!/usr/bin/env python3
"""Benchmark Kokoro TTS via PyTorch + MPS (Metal Performance Shaders) on Apple Silicon."""

import sys
import time

PHRASES = [
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
]


def check_deps():
    try:
        import torch  # noqa: F401
        if not torch.backends.mps.is_available():
            print("MPS not available on this system")
            return False
        return True
    except ImportError:
        print("PyTorch not installed. Install with:")
        print("  pip install torch kokoro")
        return False


def run_benchmark():
    if not check_deps():
        sys.exit(1)

    import torch
    from kokoro import KPipeline

    device = "mps"
    print(f"\n  PyTorch {torch.__version__}, MPS: {torch.backends.mps.is_available()}")
    print(f"  Device: {device}")

    print("  Loading model...")
    t0 = time.time()
    pipeline = KPipeline(lang_code="i", device=device)
    load_ms = (time.time() - t0) * 1000
    print(f"  Model loaded in {load_ms:.0f}ms\n")

    print(f"  {'='*60}")
    print(f"  Backend: PyTorch + MPS (Metal GPU)")
    print(f"  Model load: {load_ms:.0f}ms")
    print(f"  {'='*60}\n")

    # Warmup
    for _ in pipeline(text="Test.", voice="af", speed=1.0):
        pass

    print(f"  {'Label':<8} {'Chars':>5} {'Total':>8} {'Audio':>6} {'RTFx':>6}")
    print(f"  {'-'*40}")

    results = []
    for label, text in PHRASES:
        t = time.time()
        audio_chunks = []
        for _, _, audio in pipeline(text=text, voice="af", speed=1.0):
            audio_chunks.append(audio)
        elapsed_ms = (time.time() - t) * 1000

        total_samples = sum(a.shape[-1] for a in audio_chunks)
        audio_duration_s = total_samples / 24000.0
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
