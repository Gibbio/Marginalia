#!/usr/bin/env bash
# Build `Marginalia.app` — a macOS application bundle.
#
# Two modes:
#
#   --mock (default)  Wraps the SwiftUI preview binary (`MarginaliaUIPreview`)
#                     bound to `MockHost`. No Rust runtime, no FFI, no models
#                     required. Use for design review, UX iteration, demos.
#
#   --live            Wraps `MarginaliaUILive`, which links
#                     `MarginaliaKit.xcframework` and drives the UI via the
#                     real Rust runtime (FFIHost). Requires the xcframework
#                     to already be built — run `make build-xcframework`
#                     first, or use `make bundle-live` which chains both.
#
# Neither mode produces a signed/notarized artifact (F — blocked on the
# Apple Developer account). Both use ad-hoc signing, which is enough for
# local double-click.
#
# Usage:
#   apps/mac-gui/scripts/build-app-bundle.sh [--mock|--live]  # debug build
#   PROFILE=release ./.../build-app-bundle.sh --live          # release build
#
# Output: apps/mac-gui/build/Marginalia.app

set -euo pipefail

# ── flags ───────────────────────────────────────────────────────────────
MODE="mock"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --mock) MODE="mock"; shift ;;
    --live) MODE="live"; shift ;;
    -h|--help)
      echo "Usage: $0 [--mock|--live]"
      exit 0 ;;
    *) echo "unknown flag: $1"; exit 2 ;;
  esac
done

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$REPO_ROOT"

PROFILE="${PROFILE:-debug}"
APP_NAME="Marginalia"
OUT_DIR="apps/mac-gui/build"
APP_DIR="$OUT_DIR/$APP_NAME.app"
TEMPLATE_DIR="apps/mac-gui/Marginalia.app-template"
UI_PKG_DIR="apps/mac-gui/MarginaliaUI"
GENERATED_DIR="apps/mac-gui/Generated"
XCFRAMEWORK="$GENERATED_DIR/MarginaliaKit.xcframework"
GENERATED_SWIFT="$GENERATED_DIR/Marginalia.swift"

# ── sanity checks ──────────────────────────────────────────────────────
[[ -d "$TEMPLATE_DIR" ]] || { echo "missing template: $TEMPLATE_DIR"; exit 1; }

echo "==> Mode: $MODE"

# ── 1a. Live mode: stage the generated bindings into the package tree ──
# The xcframework's internal Swift bindings (`Marginalia.swift`) are emitted
# by `build-rust-xcframework.sh` into Generated/. SwiftPM expects them inside
# the `Marginalia` target's sources; the folder is .gitignored, we populate
# it just-in-time.
EXEC_TARGET="MarginaliaUIPreview"
if [[ "$MODE" == "live" ]]; then
  EXEC_TARGET="MarginaliaUILive"

  [[ -d "$XCFRAMEWORK" ]] || {
    echo "missing xcframework: $XCFRAMEWORK"
    echo "run: make build-xcframework"
    exit 1
  }
  [[ -f "$GENERATED_SWIFT" ]] || {
    echo "missing bindings: $GENERATED_SWIFT"
    echo "run: make build-xcframework"
    exit 1
  }

  MARGINALIA_SRC_DIR="$UI_PKG_DIR/Sources/MarginaliaKit"
  mkdir -p "$MARGINALIA_SRC_DIR"
  cp "$GENERATED_SWIFT" "$MARGINALIA_SRC_DIR/Marginalia.swift"
fi

# ── 1b. Build the Swift executable the .app wraps ──────────────────────
echo "==> Building SwiftUI $EXEC_TARGET ($PROFILE)"
(
  cd "$UI_PKG_DIR"
  if [[ "$MODE" == "live" ]]; then
    export MARGINALIA_LIVE=1
  fi
  if [[ "$PROFILE" == "release" ]]; then
    swift build --configuration release --product "$EXEC_TARGET"
  else
    swift build --product "$EXEC_TARGET"
  fi
)

BIN_PATH="$UI_PKG_DIR/.build/$PROFILE/$EXEC_TARGET"
[[ -x "$BIN_PATH" ]] || { echo "binary missing: $BIN_PATH"; exit 1; }

# ── 2. Build the pre-compiled STT helper ───────────────────────────────
echo "==> Building Swift STT helper"
make --no-print-directory build-stt-helper

STT_HELPER_BIN=$(ls -1 "$REPO_ROOT"/target/stt-helper/stt-helper-v* | head -1)
[[ -x "$STT_HELPER_BIN" ]] || { echo "stt-helper missing"; exit 1; }

# ── 3. Layout the .app bundle ──────────────────────────────────────────
echo "==> Staging $APP_DIR"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources/fonts"
mkdir -p "$APP_DIR/Contents/Resources/models/tts/mlx/voices"
mkdir -p "$APP_DIR/Contents/Helpers"
if [[ "$MODE" == "live" ]]; then
  mkdir -p "$APP_DIR/Contents/Frameworks"
fi

# 3a. Main binary.
cp "$BIN_PATH" "$APP_DIR/Contents/MacOS/$APP_NAME"

# 3b. Info.plist + entitlements.
cp "$TEMPLATE_DIR/Info.plist" "$APP_DIR/Contents/Info.plist"
cp "$TEMPLATE_DIR/Marginalia.entitlements" "$APP_DIR/Contents/Resources/Marginalia.entitlements"

# 3c. STT helper.
cp "$STT_HELPER_BIN" "$APP_DIR/Contents/Helpers/"

# 3d. Fonts — SwiftPM produces a resource bundle, but we also drop the raw
# .ttf files at Resources/fonts/ to match ATSApplicationFontsPath in Info.plist.
# (The bundled Bundle.module already has them, but that path is undiscoverable
# from Info.plist; the double-host is cheap and future-proof.)
cp "$UI_PKG_DIR"/Sources/MarginaliaUI/Resources/fonts/*.ttf "$APP_DIR/Contents/Resources/fonts/" 2>/dev/null || true

# Copy the SwiftPM resource bundle so Bundle.module keeps working.
RESOURCE_BUNDLE=$(find "$UI_PKG_DIR/.build/$PROFILE" -maxdepth 2 -name "MarginaliaUI_MarginaliaUI.bundle" -print -quit || true)
if [[ -n "$RESOURCE_BUNDLE" ]]; then
  cp -R "$RESOURCE_BUNDLE" "$APP_DIR/Contents/Resources/"
fi

# 3e. Kokoro MLX model + voice manifest, if present. (Absence triggers
# the onboarding flow at first launch — Workstream L.)
if [[ -f models/tts/mlx/kokoro-v1_0.safetensors ]]; then
  cp models/tts/mlx/kokoro-v1_0.safetensors "$APP_DIR/Contents/Resources/models/tts/mlx/"
fi
if [[ -f models/tts/mlx/voices.manifest.json ]]; then
  cp models/tts/mlx/voices.manifest.json "$APP_DIR/Contents/Resources/models/tts/mlx/"
fi
for voice in models/tts/mlx/voices/*.safetensors; do
  [[ -f "$voice" ]] || continue
  cp "$voice" "$APP_DIR/Contents/Resources/models/tts/mlx/voices/"
done

# 3f. Licences that travel with the app (OFL for the three fonts; Apache-2.0
# for Kokoro; BSD-3 / MIT for AEC3 / whisper.cpp / espeak-ng when those end up
# shipping). Collected for the future Acknowledgements view.
mkdir -p "$APP_DIR/Contents/Resources/licenses"
cp "$UI_PKG_DIR/Sources/MarginaliaUI/Resources/fonts/OFL-"*.txt "$APP_DIR/Contents/Resources/licenses/" 2>/dev/null || true

# 3g. App icon.
# Prefers a hand-made source at Marginalia.app-template/AppIcon.png (any
# square size ≥ 512). Falls back to a warm-dark placeholder PNG generated
# on the fly so Finder at least gets a non-default Dock glyph.
if command -v sips >/dev/null && command -v iconutil >/dev/null; then
  SOURCE_PNG="$TEMPLATE_DIR/AppIcon.png"
  CLEANUP_SOURCE=""
  if [[ ! -f "$SOURCE_PNG" ]]; then
    echo "  (AppIcon.png not found in template; generating warm-dark placeholder)"
    SOURCE_PNG="$(mktemp -t marginalia-icon).png"
    CLEANUP_SOURCE="$SOURCE_PNG"
    python3 - "$SOURCE_PNG" <<'PY'
import sys, struct, zlib
W = H = 1024
def png_ihdr(w, h):
    return struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)
def chunk(tag, data):
    out = struct.pack(">I", len(data)) + tag + data
    out += struct.pack(">I", zlib.crc32(tag + data) & 0xffffffff)
    return out
row = bytes([0]) + bytes([0x0F, 0x0E, 0x10]) * W
raw = row * H
idat = zlib.compress(raw, 9)
out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", png_ihdr(W,H)) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")
with open(sys.argv[1], "wb") as f: f.write(out)
PY
  else
    echo "  Using custom icon: $SOURCE_PNG"
  fi

  ICONSET_DIR="$(mktemp -d -t marginalia-iconset).iconset"
  rm -rf "$ICONSET_DIR"; mkdir -p "$ICONSET_DIR"
  for sz in 16 32 128 256 512; do
    sips -z $sz $sz "$SOURCE_PNG" --out "$ICONSET_DIR/icon_${sz}x${sz}.png" >/dev/null
    dbl=$((sz * 2))
    sips -z $dbl $dbl "$SOURCE_PNG" --out "$ICONSET_DIR/icon_${sz}x${sz}@2x.png" >/dev/null
  done
  iconutil -c icns -o "$APP_DIR/Contents/Resources/AppIcon.icns" "$ICONSET_DIR" 2>/dev/null || \
    echo "  (iconutil failed; continuing without custom icon)"
  rm -rf "$ICONSET_DIR"
  [[ -n "$CLEANUP_SOURCE" ]] && rm -f "$CLEANUP_SOURCE"
fi

# 3h. Live mode: copy the arm64 slice's .framework into the .app's
# Frameworks/ dir so dyld finds MarginaliaFFI.framework/MarginaliaFFI at
# launch (the dylib's install_name is @rpath/MarginaliaFFI.framework/...,
# and SwiftPM-linked binaries have @executable_path/../Frameworks on their
# LC_RPATH, so this is the default search location).
if [[ "$MODE" == "live" ]]; then
  SLICE_FRAMEWORK="$XCFRAMEWORK/macos-arm64/MarginaliaFFI.framework"
  [[ -d "$SLICE_FRAMEWORK" ]] || {
    echo "missing framework slice: $SLICE_FRAMEWORK"
    echo "run: make build-xcframework"
    exit 1
  }
  cp -R "$SLICE_FRAMEWORK" "$APP_DIR/Contents/Frameworks/"
fi

# ── 4. Ad-hoc code sign ───────────────────────────────────────────────
# This makes Gatekeeper treat it as "signed by no authority" rather than
# "unsigned", which is enough for local double-click. Replace with
# `--sign "Developer ID Application: …"` when F kicks off.
echo "==> Ad-hoc signing"
# Sign helpers first, then the whole bundle (deep).
codesign --force --sign - "$APP_DIR/Contents/Helpers/"stt-helper-v*
codesign --force --deep --sign - \
  --entitlements "$APP_DIR/Contents/Resources/Marginalia.entitlements" \
  --options runtime \
  "$APP_DIR" 2>&1 | sed 's/^/  /'

# ── 5. Point the binary at the bundled helper via env var at launch ───
# A minimal launcher stub wraps the actual binary — harder-to-misdirect than
# setting the env var from the runtime itself, and keeps main Swift code
# unaware of packaging concerns.
LAUNCHER="$APP_DIR/Contents/MacOS/$APP_NAME"
STUB="$(mktemp)"
cat > "$STUB" <<'EOF'
#!/bin/bash
# Launcher stub: point the STT helper resolver at our bundled copy, then exec.
BUNDLE_MACOS="$(dirname "$0")"
APP_BUNDLE="$(cd "$BUNDLE_MACOS/.." && pwd)"
HELPER=$(ls -1 "$APP_BUNDLE/Helpers/stt-helper-v"* 2>/dev/null | head -1)
[[ -n "$HELPER" ]] && export MARGINALIA_STT_HELPER="$HELPER"
exec "$BUNDLE_MACOS/Marginalia.bin" "$@"
EOF
mv "$LAUNCHER" "$LAUNCHER.bin"
mv "$STUB" "$LAUNCHER"
chmod +x "$LAUNCHER"
# Re-sign the wrapped binary so Gatekeeper doesn't notice the swap.
codesign --force --sign - "$LAUNCHER.bin"
codesign --force --sign - "$LAUNCHER"

# ── 6. Report ─────────────────────────────────────────────────────────
echo ""
echo "Done."
echo "  mode:   $MODE"
echo "  .app:   $APP_DIR"
echo "  size:   $(du -sh "$APP_DIR" | cut -f1)"
echo ""
echo "Open with: open \"$APP_DIR\""
