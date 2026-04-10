#!/usr/bin/env python3
"""Benchmark MeloTTS on Apple Silicon."""

import sys
import time

PHRASES = [
    ("tiny",   "La stanza era silenziosa."),
    ("short",  "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra."),
    ("medium", "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco."),
    ("long",   "La stanza era silenziosa, ma non immobile. Un leggero fruscio proveniva dalla finestra socchiusa, dove la brezza notturna faceva ondeggiare la tenda di lino bianco. Sul tavolo, accanto a una tazza di tè ormai freddo, giaceva un libro aperto a metà, le pagine ingiallite illuminate dalla luce calda di una lampada."),
]

SAMPLE_RATE = 44100  # MeloTTS default


def run_benchmark():
    try:
        from melo.api import TTS
    except ImportError:
        print("  SKIP: MeloTTS not installed")
        print("  Install: pip install git+https://github.com/myshell-ai/MeloTTS.git")
        sys.exit(1)

    # MeloTTS doesn't have Italian — use English as quality baseline
    print("\n  Loading MeloTTS model (English)...")
    t0 = time.time()
    model = TTS(language="EN", device="auto")
    load_ms = (time.time() - t0) * 1000
    print(f"  Model loaded in {load_ms:.0f}ms")
    print(f"  Device: {model.device}")

    speaker_ids = model.hps.data.spk2id
    speaker = list(speaker_ids.keys())[0]
    speaker_id = speaker_ids[speaker]

    print(f"\n  {'='*60}")
    print(f"  Backend: MeloTTS (PyTorch, device={model.device})")
    print(f"  Speaker: {speaker}")
    print(f"  Model load: {load_ms:.0f}ms")
    print(f"  {'='*60}")

    # Use English phrases since MeloTTS doesn't support Italian
    en_phrases = [
        ("tiny",   "The room was quiet."),
        ("short",  "The room was quiet, but not still. A faint rustle came from the half-open window."),
        ("medium", "The room was quiet, but not still. A faint rustle came from the half-open window, where the night breeze swayed the white linen curtain. On the table lay an open book."),
        ("long",   "The room was quiet, but not still. A faint rustle came from the half-open window, where the night breeze swayed the white linen curtain. On the table, next to a cup of cold tea, lay a half-open book, its yellowed pages lit by the warm glow of a lamp. She had been reading for hours."),
    ]

    # Warmup
    print("\n  Warmup...", end="", flush=True)
    import tempfile, os
    tmp = tempfile.mktemp(suffix=".wav")
    model.tts_to_file("Test.", speaker_id, tmp, speed=1.0)
    os.unlink(tmp)
    print(" done")

    print(f"\n  {'Label':<8} {'Chars':>5} {'Total':>8} {'Audio':>6} {'RTFx':>6}")
    print(f"  {'-'*40}")

    results = []
    for label, text in en_phrases:
        tmp = tempfile.mktemp(suffix=".wav")
        t = time.time()
        model.tts_to_file(text, speaker_id, tmp, speed=1.0)
        elapsed_ms = (time.time() - t) * 1000

        # Read wav to get duration
        import wave
        with wave.open(tmp, "r") as wf:
            audio_duration_s = wf.getnframes() / wf.getframerate()
        os.unlink(tmp)

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
    print("\n  Note: MeloTTS tested with English (no Italian support)")


if __name__ == "__main__":
    run_benchmark()
