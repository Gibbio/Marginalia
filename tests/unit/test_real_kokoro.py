"""Real Kokoro adapter tests."""

from __future__ import annotations

import sys
import textwrap
from pathlib import Path

from marginalia_adapters.real.kokoro import KokoroSpeechSynthesizer
from marginalia_core.ports.tts import SynthesisRequest


_FAKE_WORKER = textwrap.dedent("""\
    from __future__ import annotations
    import argparse, json, sys
    from pathlib import Path

    parser = argparse.ArgumentParser()
    parser.add_argument("--lang-code", required=True)
    parser.add_argument("--voice", required=True)
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--sample-rate", type=int, default=24000)
    parser.add_argument("--output-path", default=None)
    parser.add_argument("--serve", action="store_true")
    args = parser.parse_args()

    if args.serve:
        sys.stdout.write(json.dumps({"status": "ready"}) + "\\n")
        sys.stdout.flush()
        for line in sys.stdin:
            req = json.loads(line.strip())
            Path(req["output_path"]).write_text(req["text"], encoding="utf-8")
            sys.stdout.write(json.dumps({"status": "ok", "output_path": req["output_path"]}) + "\\n")
            sys.stdout.flush()
    else:
        Path(args.output_path).write_bytes(sys.stdin.read().encode("utf-8"))
""")


def test_kokoro_synthesizer_uses_external_python_worker(tmp_path: Path) -> None:
    worker_script = tmp_path / "worker.py"
    worker_script.write_text(_FAKE_WORKER, encoding="utf-8")

    synthesizer = KokoroSpeechSynthesizer(
        python_executable=sys.executable,
        output_dir=tmp_path / "audio",
        lang_code="i",
        speed=1.0,
        worker_script=worker_script,
    )

    result = synthesizer.synthesize(SynthesisRequest(text="Ciao dal test.", voice="if_sara"))

    assert result.provider_name == "kokoro"
    assert result.voice == "if_sara"
    assert result.metadata["lang_code"] == "i"
    assert Path(result.audio_reference).exists()

    # Second call reuses the persistent worker (no model reload).
    result2 = synthesizer.synthesize(SynthesisRequest(text="Secondo chunk.", voice="if_sara"))
    assert Path(result2.audio_reference).exists()
    assert result2.audio_reference != result.audio_reference

    synthesizer.shutdown()
