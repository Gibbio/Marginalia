#!/usr/bin/env bash
set -euo pipefail

export PYTHONPATH="apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src"
export MARGINALIA_DB_PATH="${MARGINALIA_DB_PATH:-$(mktemp -t marginalia-smoke)}"
export MARGINALIA_FAKE_COMMANDS="${MARGINALIA_FAKE_COMMANDS:-pausa,continua,stop}"
export MARGINALIA_TTS_PROVIDER="${MARGINALIA_TTS_PROVIDER:-fake}"
export MARGINALIA_PLAYBACK_PROVIDER="${MARGINALIA_PLAYBACK_PROVIDER:-fake}"
export MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS="${MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS:-2}"

.venv/bin/python -m marginalia_cli doctor --json
.venv/bin/python -m marginalia_cli play examples/voice-test-it.txt --json
.venv/bin/python -m marginalia_cli status --json
MARGINALIA_FAKE_COMMANDS="" MARGINALIA_FAKE_PLAYBACK_AUTO_COMPLETE_POLLS=0 \
  .venv/bin/python -m marginalia_cli play examples/voice-test-it.txt --json
.venv/bin/python -m marginalia_cli status --json
