#!/usr/bin/env bash
# Build phase — ensure `MarginaliaKit.xcframework` + UniFFI bindings exist
# before Xcode starts linking. Runs before "Compile Sources" so the
# MarginaliaUI SwiftPM package (which auto-detects live mode via the
# bindings file presence) picks up the FFI-enabled build.
#
# Rebuilds ONLY when the xcframework or bindings file is missing. We don't
# watch for Rust source changes here — the mid-build rewrite would pull the
# rug out from under Xcode's already-resolved dependency graph. To pick up
# Rust changes, run `make build-xcframework` outside Xcode.
#
# Missing framework is an Xcode-opened-on-a-fresh-clone scenario: do a
# debug-profile rebuild so the project is buildable on first run.

set -euo pipefail

# ${PROJECT_DIR} is apps/mac-gui/ when running under xcodebuild. From standalone
# invocation it's unset, and $0 lives at apps/mac-gui/Marginalia/build-scripts/,
# which is 4 levels below the repo root.
if [[ -n "${PROJECT_DIR:-}" ]]; then
    REPO_ROOT="$PROJECT_DIR/../.."
else
    REPO_ROOT="$(dirname "$0")/../../../.."
fi
REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"

XCFRAMEWORK="$REPO_ROOT/apps/mac-gui/Generated/MarginaliaKit.xcframework"
BINDINGS="$REPO_ROOT/apps/mac-gui/Generated/Marginalia.swift"
# Mirror of the bindings into the SwiftPM package tree — the file that
# Package.swift probes to enable live mode.
PKG_BINDINGS="$REPO_ROOT/apps/mac-gui/MarginaliaUI/Sources/MarginaliaKit/Marginalia.swift"

if [[ ! -d "$XCFRAMEWORK" || ! -f "$BINDINGS" ]]; then
    echo "preflight-xcframework: xcframework or bindings missing — building"
    PROFILE="${MARGINALIA_RUST_PROFILE:-${CONFIGURATION:-Release}}"
    PROFILE="$(echo "$PROFILE" | tr '[:upper:]' '[:lower:]')"
    [[ "$PROFILE" == "debug" ]] || PROFILE="release"
    cd "$REPO_ROOT"
    PROFILE="$PROFILE" apps/mac-gui/scripts/build-rust-xcframework.sh
else
    echo "preflight-xcframework: xcframework present — skipping rebuild"
    echo "    (run \`make build-xcframework\` outside Xcode to pick up Rust changes)"
fi

# Always mirror bindings into the SwiftPM package tree (cheap).
mkdir -p "$(dirname "$PKG_BINDINGS")"
cp -f "$BINDINGS" "$PKG_BINDINGS"
