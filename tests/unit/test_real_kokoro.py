"""Real Kokoro adapter tests."""

from __future__ import annotations

import sys
from pathlib import Path

from marginalia_adapters.real.kokoro import KokoroSpeechSynthesizer
from marginalia_core.ports.tts import SynthesisRequest


def test_kokoro_synthesizer_uses_external_python_worker(tmp_path: Path) -> None:
    worker_script = tmp_path / "worker.py"
    worker_script.write_text(
        "\n".join(
            [
                "from __future__ import annotations",
                "import argparse",
                "import sys",
                "from pathlib import Path",
                "",
                "parser = argparse.ArgumentParser()",
                "parser.add_argument('--output-path', required=True)",
                "parser.add_argument('--lang-code', required=True)",
                "parser.add_argument('--voice', required=True)",
                "parser.add_argument('--speed', required=True)",
                "args = parser.parse_args()",
                "Path(args.output_path).write_bytes(sys.stdin.read().encode('utf-8'))",
                "",
            ]
        ),
        encoding="utf-8",
    )

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
