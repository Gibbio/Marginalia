#!/usr/bin/env bash
# Build phase — copy non-source artefacts into the staged `.app` bundle:
#   • STT helper binary  → Contents/Helpers/
#   • Variable fonts     → Contents/Resources/fonts/
#   • Licenses           → Contents/Resources/licenses/
#
# Models intentionally NOT copied — they live in
# `~/Library/Application Support/Marginalia/models/` and are downloaded
# by the onboarding flow. Keeping them out of the bundle keeps the `.app`
# ~15 MB instead of ~500 MB.
#
# Runs under xcodebuild with ${TARGET_BUILD_DIR} / ${UNLOCALIZED_RESOURCES_FOLDER_PATH}
# etc. already set.

set -euo pipefail

if [[ -n "${PROJECT_DIR:-}" ]]; then
    REPO_ROOT="$PROJECT_DIR/../.."
else
    REPO_ROOT="$(dirname "$0")/../../../.."
fi
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

APP_BUNDLE="${TARGET_BUILD_DIR:?TARGET_BUILD_DIR is required}/${FULL_PRODUCT_NAME:?FULL_PRODUCT_NAME is required}"
CONTENTS="$APP_BUNDLE/Contents"

mkdir -p "$CONTENTS/Helpers"
mkdir -p "$CONTENTS/Resources/fonts"
mkdir -p "$CONTENTS/Resources/licenses"

# 1. STT helper — pick the newest stt-helper-v* produced by `make build-stt-helper`.
HELPER=$(ls -1t "$REPO_ROOT"/target/stt-helper/stt-helper-v* 2>/dev/null | head -1 || true)
if [[ -n "$HELPER" && -x "$HELPER" ]]; then
    echo "stage-resources: copying $(basename "$HELPER")"
    cp "$HELPER" "$CONTENTS/Helpers/"
else
    echo "stage-resources: warning — no STT helper found; STT will fall back to compile-on-first-launch (needs Xcode CLT on host)."
fi

# 2. Fonts — the SwiftPM MarginaliaUI resource bundle already ships them via
# Bundle.module, but mirroring to Contents/Resources/fonts/ satisfies the
# ATSApplicationFontsPath key in Info.plist (which can't read Bundle.module).
FONT_SRC="$REPO_ROOT/apps/mac-gui/MarginaliaUI/Sources/MarginaliaUI/Resources/fonts"
if [[ -d "$FONT_SRC" ]]; then
    cp "$FONT_SRC"/*.ttf "$CONTENTS/Resources/fonts/" 2>/dev/null || true
fi

# 3. Licenses that travel with the app (OFL for the three font families).
#    Room to add Apache-2.0 / MIT / BSD-3 for Kokoro / whisper.cpp / AEC3
#    once they ship bundled weights — currently the user installs those
#    at runtime and the licenses live next to the downloaded files.
cp "$FONT_SRC"/OFL-*.txt "$CONTENTS/Resources/licenses/" 2>/dev/null || true

echo "stage-resources: done"
