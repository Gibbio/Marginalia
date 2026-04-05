#!/usr/bin/env bash
set -euo pipefail

export PYTHONPATH="apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src"
export MARGINALIA_DB_PATH="${MARGINALIA_DB_PATH:-$(mktemp -t marginalia-smoke)}"
export MARGINALIA_FAKE_COMMANDS="${MARGINALIA_FAKE_COMMANDS:-pausa,continua}"
export MARGINALIA_TTS_PROVIDER="${MARGINALIA_TTS_PROVIDER:-fake}"
export MARGINALIA_PLAYBACK_PROVIDER="${MARGINALIA_PLAYBACK_PROVIDER:-fake}"

.venv/bin/python -m marginalia_cli doctor --json
.venv/bin/python -m marginalia_cli ingest examples/sample-document.txt --json
.venv/bin/python -m marginalia_cli play --json
.venv/bin/python -m marginalia_cli repeat --json
.venv/bin/python -m marginalia_cli next-chapter --json
.venv/bin/python -m marginalia_cli restart-chapter --json
.venv/bin/python -m marginalia_cli pause --json
.venv/bin/python -m marginalia_cli resume --json
.venv/bin/python -m marginalia_cli control-loop --max-commands 2 --json
.venv/bin/python -m marginalia_cli pause --json
.venv/bin/python -m marginalia_cli note-start --json
.venv/bin/python -m marginalia_cli note-stop --text "Review the opening paragraph." --json
.venv/bin/python -m marginalia_cli rewrite-current --json
.venv/bin/python -m marginalia_cli summarize-topic local --json
.venv/bin/python -m marginalia_cli search-document local --json
.venv/bin/python -m marginalia_cli search-notes opening --json
.venv/bin/python -m marginalia_cli status --json
