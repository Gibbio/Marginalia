#!/usr/bin/env bash
# Build phase — make sure the pre-compiled Swift STT helper exists at
# `target/stt-helper/stt-helper-vN`. `stage-resources.sh` then copies it
# into `Contents/Helpers/` in the app bundle.
#
# The underlying `make build-stt-helper` target is a no-op when the binary
# is already present and up-to-date, so re-running is cheap.

set -euo pipefail

if [[ -n "${PROJECT_DIR:-}" ]]; then
    REPO_ROOT="$PROJECT_DIR/../.."
else
    REPO_ROOT="$(dirname "$0")/../../../.."
fi
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

cd "$REPO_ROOT"
make --no-print-directory build-stt-helper
