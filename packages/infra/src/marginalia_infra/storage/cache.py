"""Audio cache management."""

from __future__ import annotations

import logging
import time
from pathlib import Path

logger = logging.getLogger(__name__)


def cleanup_audio_cache(cache_dir: Path, *, max_age_hours: int = 72) -> int:
    """Remove WAV files older than *max_age_hours* and return the count deleted."""

    if not cache_dir.exists():
        return 0

    cutoff = time.time() - (max_age_hours * 3600)
    removed = 0
    for path in cache_dir.glob("*.wav"):
        try:
            if path.stat().st_mtime < cutoff:
                path.unlink()
                removed += 1
        except OSError:
            continue

    if removed:
        logger.info("Cleaned %d stale audio artifact(s) from %s", removed, cache_dir)
    return removed
