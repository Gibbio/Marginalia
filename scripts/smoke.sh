#!/usr/bin/env bash
set -euo pipefail

export PYTHONPATH="apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src"
export MARGINALIA_DB_PATH="${MARGINALIA_DB_PATH:-.marginalia/smoke.sqlite3}"

.venv/bin/python -m marginalia_cli doctor --json
.venv/bin/python -m marginalia_cli ingest examples/sample-document.txt --json
.venv/bin/python -m marginalia_cli status --json
