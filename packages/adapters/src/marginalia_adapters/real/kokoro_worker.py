"""Standalone Kokoro synthesis worker for a dedicated Python runtime.

Supports two modes:

- **one-shot** (legacy): reads text from stdin, writes one WAV, exits.
- **persistent** (``--serve``): stays alive, reads JSON requests from stdin
  line by line, writes JSON responses to stdout.  The Kokoro model is loaded
  once and reused across requests.
"""

from __future__ import annotations

import argparse
import json
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
    parser.add_argument("--output-path", default=None)
    parser.add_argument("--lang-code", required=True)
    parser.add_argument("--voice", required=True)
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--sample-rate", type=int, default=24_000)
    parser.add_argument("--serve", action="store_true", help="Run as persistent worker.")
    return parser.parse_args(argv)


def _load_deps() -> tuple[Any, Any, Any]:
    _remove_local_adapter_path()
    try:
        import numpy as np  # type: ignore[import-not-found]
        import soundfile as sf  # type: ignore[import-not-found]
        from kokoro import KPipeline  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit(f"Kokoro worker dependency missing: {exc}") from exc
    return np, sf, KPipeline


def _torch_runtime_info() -> dict[str, str]:
    try:
        import torch  # type: ignore[import-not-found]
    except ImportError:
        return {"backend": "unknown", "acceleration": "unavailable"}

    mps_backend = getattr(torch.backends, "mps", None)
    mps_available = bool(mps_backend and mps_backend.is_available())
    cuda_available = bool(torch.cuda.is_available())

    if mps_available:
        return {"backend": "mps", "acceleration": "enabled"}
    if cuda_available:
        return {"backend": "cuda", "acceleration": "enabled"}
    return {"backend": "cpu", "acceleration": "disabled"}


def _synthesize(
    pipeline: Any,
    np: Any,
    sf: Any,
    text: str,
    voice: str,
    speed: float,
    sample_rate: int,
    output_path: Path,
) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    segments: list[Any] = []
    for _, _, audio in pipeline(text, voice=voice, speed=speed, split_pattern=r"\n+"):
        segments.append(np.asarray(audio))
    if not segments:
        raise RuntimeError("Kokoro worker produced no audio segments.")
    sf.write(output_path, np.concatenate(segments), sample_rate)


def _serve(args: argparse.Namespace) -> int:
    """Persistent mode: one model load, many synthesis requests."""

    np, sf, KPipeline = _load_deps()
    pipeline = KPipeline(lang_code=args.lang_code)
    ready_payload = {"status": "ready", **_torch_runtime_info()}
    sys.stdout.write(json.dumps(ready_payload) + "\n")
    sys.stdout.flush()

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            sys.stdout.write(json.dumps({"status": "error", "message": str(exc)}) + "\n")
            sys.stdout.flush()
            continue

        text = request.get("text", "").strip()
        if not text:
            sys.stdout.write(
                json.dumps({"status": "error", "message": "No text provided."}) + "\n"
            )
            sys.stdout.flush()
            continue

        output_path = Path(request["output_path"])
        voice = request.get("voice", args.voice)
        speed = request.get("speed", args.speed)
        sample_rate = request.get("sample_rate", args.sample_rate)

        try:
            _synthesize(pipeline, np, sf, text, voice, speed, sample_rate, output_path)
            sys.stdout.write(json.dumps({"status": "ok", "output_path": str(output_path)}) + "\n")
        except Exception as exc:
            sys.stdout.write(json.dumps({"status": "error", "message": str(exc)}) + "\n")
        sys.stdout.flush()

    return 0


def _one_shot(args: argparse.Namespace) -> int:
    """Legacy one-shot mode: read stdin text, write one WAV, exit."""

    text = sys.stdin.read().strip()
    if not text:
        raise SystemExit("No input text was provided to the Kokoro worker.")

    np, sf, KPipeline = _load_deps()
    output_path = Path(args.output_path)
    pipeline = KPipeline(lang_code=args.lang_code)
    _synthesize(pipeline, np, sf, text, args.voice, args.speed, args.sample_rate, output_path)
    return 0


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv or sys.argv[1:])
    if args.serve:
        return _serve(args)
    if not args.output_path:
        raise SystemExit("--output-path is required in one-shot mode.")
    return _one_shot(args)


if __name__ == "__main__":
    raise SystemExit(main())
