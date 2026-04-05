"""Shared test helpers."""

from __future__ import annotations

import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE_ROOTS = (
    REPO_ROOT / "apps" / "cli" / "src",
    REPO_ROOT / "packages" / "core" / "src",
    REPO_ROOT / "packages" / "adapters" / "src",
    REPO_ROOT / "packages" / "infra" / "src",
)

for source_root in SOURCE_ROOTS:
    sys.path.insert(0, str(source_root))
