"""Standalone Kokoro synthesis worker for a dedicated Python runtime."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path
from typing import Any


def _remove_local_adapter_path() -> None:
    worker_dir = Path(__file__).resolve().parent
    filtered_paths: list[str] = []
    for entry in sys.path:
        try:
            if Path(entry).resolve() == worker_dir:
                continue
        except OSError:
            pass
        filtered_paths.append(entry)
    sys.path[:] = filtered_paths


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Synthesize speech through Kokoro.")
    parser.add_argument("--output-path", required=True)
    parser.add_argument("--lang-code", required=True)
    parser.add_argument("--voice", required=True)
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--sample-rate", type=int, default=24_000)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv or sys.argv[1:])
    text = sys.stdin.read().strip()
    if not text:
        raise SystemExit("No input text was provided to the Kokoro worker.")

    _remove_local_adapter_path()
    try:
        import numpy as np
        import soundfile as sf  # type: ignore[import-not-found]
        from kokoro import KPipeline  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit(f"Kokoro worker dependency missing: {exc}") from exc

    output_path = Path(args.output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    pipeline = KPipeline(lang_code=args.lang_code)
    segments: list[Any] = []
    for _, _, audio in pipeline(text, voice=args.voice, speed=args.speed, split_pattern=r"\n+"):
        segments.append(np.asarray(audio))

    if not segments:
        raise SystemExit("Kokoro worker produced no audio segments.")

    sf.write(output_path, np.concatenate(segments), args.sample_rate)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
