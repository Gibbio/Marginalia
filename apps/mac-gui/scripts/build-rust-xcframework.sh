#!/usr/bin/env bash
# Build `MarginaliaKit.xcframework` — the Rust runtime packaged for Swift.
#
# What this does:
#   1. Compile `marginalia-ffi` as BOTH a staticlib and a cdylib for arm64
#      macOS. Only the cdylib ships — we use it because cargo's `staticlib`
#      crate-type produces archives with duplicate object-file entries
#      (e.g. two copies of `export.cpp.o` — one stripped from the runtime
#      build, one full from libmlx.a) that `ar`/`libtool` can't merge
#      deterministically. The cdylib has no such problems: rustc fully
#      links it at compile time and every symbol is resolved and deduped
#      correctly in the final image.
#   2. Rewrite the dylib's install_name to `@rpath/MarginaliaFFI.framework/MarginaliaFFI`
#      so it can be loaded from inside an `.app` bundle.
#   3. Wrap it as a `MarginaliaFFI.framework` (standard macOS bundle layout).
#   4. Generate Swift bindings via `uniffi-bindgen`.
#   5. Assemble an `xcframework` for SwiftPM / Xcode consumption.
#
# Outputs go to `apps/mac-gui/Generated/`:
#   - MarginaliaKit.xcframework/  (linked into the Swift build)
#   - Marginalia.swift            (copied into the live target's sources)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
OUT_DIR="${REPO_ROOT}/apps/mac-gui/Generated"
XCFRAMEWORK_NAME="MarginaliaKit"
FRAMEWORK_NAME="MarginaliaFFI"     # module name imported in Swift (`import MarginaliaFFI`)
DYLIB_NAME="libmarginalia_ffi.dylib"

FEATURES="${FEATURES:-mlx-tts,apple-stt,whisper-stt,host-playback}"
PROFILE="${PROFILE:-release}"
# Apple Silicon only — MLX requires Metal on arm64, and we don't ship a
# non-MLX path for macOS. Intel Macs aren't a supported target.
TARGETS="aarch64-apple-darwin"

cd "$REPO_ROOT"

HOST_ARCH="$(uname -m)"
if [[ "$HOST_ARCH" != "arm64" ]]; then
    echo "error: Marginalia macOS builds require Apple Silicon (got $HOST_ARCH)."
    exit 1
fi

echo "==> Ensuring Rust targets are installed"
for t in $TARGETS; do
    rustup target add "$t" >/dev/null
done

echo "==> Building marginalia-ffi [${FEATURES}] for ${TARGETS}"
# macOS ships bash 3.2 where `"${FLAGS[@]}"` on an empty array trips `set -u`.
# Use the `${var[@]+…}` pattern to expand only when set.
FLAGS=()
[[ "$PROFILE" == "release" ]] && FLAGS+=(--release)
for t in $TARGETS; do
    cargo build -p marginalia-ffi --features "$FEATURES" --target "$t" ${FLAGS[@]+"${FLAGS[@]}"}
done

echo "==> Generating Swift bindings via uniffi-bindgen"
# Run bindgen with the host build of the library so we can load its metadata.
# The resulting .swift + .h + .modulemap don't depend on the target arch.
GEN_DIR="${OUT_DIR}/bindings"
rm -rf "$GEN_DIR"
mkdir -p "$GEN_DIR"
HOST_LIB="target/${PROFILE}/libmarginalia_ffi.dylib"
if [[ ! -f "$HOST_LIB" ]]; then
    echo "    (building host dylib for bindgen metadata)"
    cargo build -p marginalia-ffi --features "$FEATURES" ${FLAGS[@]+"${FLAGS[@]}"}
fi
cargo run -p marginalia-ffi --bin uniffi-bindgen -- \
    generate --library "$HOST_LIB" \
    --language swift \
    --out-dir "$GEN_DIR"

# Build a standard macOS `.framework` for each target arch, then combine
# them into an `.xcframework` via `xcodebuild -create-xcframework`.
echo "==> Assembling ${XCFRAMEWORK_NAME}.xcframework"
XCFRAMEWORK_DIR="${OUT_DIR}/${XCFRAMEWORK_NAME}.xcframework"
rm -rf "$XCFRAMEWORK_DIR"

XCARGS=()
for t in $TARGETS; do
    SRC_DYLIB="target/${t}/${PROFILE}/${DYLIB_NAME}"
    [[ -f "$SRC_DYLIB" ]] || { echo "missing $SRC_DYLIB"; exit 1; }

    # Build the framework bundle at `Generated/stage-<target>/<name>.framework`.
    STAGE="${OUT_DIR}/stage-${t}"
    rm -rf "$STAGE"
    FRAMEWORK_DIR="${STAGE}/${FRAMEWORK_NAME}.framework"

    # Hierarchical (macOS-canonical) framework layout. Xcode's
    # "Validate Application" step (which runs automatically when a
    # framework is embedded into a .app) rejects flat-layout frameworks
    # with: "expected Versions/Current/Resources/Info.plist since the
    # platform does not use shallow bundles".
    #
    # Layout:
    #   MarginaliaFFI.framework/
    #     MarginaliaFFI         → Versions/Current/MarginaliaFFI
    #     Headers               → Versions/Current/Headers
    #     Resources             → Versions/Current/Resources
    #     Modules               → Versions/Current/Modules
    #     Versions/
    #       A/
    #         MarginaliaFFI (the dylib)
    #         Headers/…
    #         Resources/Info.plist
    #         Modules/module.modulemap
    #       Current            → A
    VERSION_DIR="${FRAMEWORK_DIR}/Versions/A"
    mkdir -p "${VERSION_DIR}/Headers"
    mkdir -p "${VERSION_DIR}/Resources"
    mkdir -p "${VERSION_DIR}/Modules"

    cp "$SRC_DYLIB" "${VERSION_DIR}/${FRAMEWORK_NAME}"
    cp "${GEN_DIR}/${FRAMEWORK_NAME}.h" "${VERSION_DIR}/Headers/"

    # modulemap: route `import MarginaliaFFI` at the C module that wraps
    # the header bundled in this framework. Path is relative to the
    # Modules/ dir.
    cat > "${VERSION_DIR}/Modules/module.modulemap" <<EOF
framework module ${FRAMEWORK_NAME} {
    umbrella header "${FRAMEWORK_NAME}.h"
    export *
    module * { export * }
}
EOF

    # Info.plist: minimal but enough to satisfy codesign / Gatekeeper.
    # Bundle identifier must be unique per framework name.
    cat > "${VERSION_DIR}/Resources/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>    <string>en</string>
    <key>CFBundleExecutable</key>           <string>${FRAMEWORK_NAME}</string>
    <key>CFBundleIdentifier</key>           <string>com.gibbio.marginalia.${FRAMEWORK_NAME}</string>
    <key>CFBundleName</key>                 <string>${FRAMEWORK_NAME}</string>
    <key>CFBundlePackageType</key>          <string>FMWK</string>
    <key>CFBundleShortVersionString</key>   <string>0.1.0</string>
    <key>CFBundleVersion</key>              <string>1</string>
    <key>MinimumOSVersion</key>             <string>14.0</string>
    <key>LSMinimumSystemVersion</key>       <string>14.0</string>
</dict>
</plist>
EOF

    # Symlinks at the framework root pointing into Versions/Current.
    ln -sf "A" "${FRAMEWORK_DIR}/Versions/Current"
    ln -sf "Versions/Current/${FRAMEWORK_NAME}" "${FRAMEWORK_DIR}/${FRAMEWORK_NAME}"
    ln -sf "Versions/Current/Headers"   "${FRAMEWORK_DIR}/Headers"
    ln -sf "Versions/Current/Resources" "${FRAMEWORK_DIR}/Resources"
    ln -sf "Versions/Current/Modules"   "${FRAMEWORK_DIR}/Modules"

    # Rewrite the dylib's install_name so dyld can find it inside the
    # consumer .app's Frameworks/ directory via @rpath. The consumer's
    # LC_RPATH must include `@executable_path/../Frameworks` — macOS
    # bundles set that up automatically when linking via SwiftPM.
    install_name_tool -id \
        "@rpath/${FRAMEWORK_NAME}.framework/Versions/A/${FRAMEWORK_NAME}" \
        "${VERSION_DIR}/${FRAMEWORK_NAME}"

    # Ad-hoc code-sign so Gatekeeper / codesign --verify is happy when
    # the consumer bundle re-signs with --deep. Sign the versioned dylib
    # (not the root symlink — codesign won't follow symlinks) then the
    # whole framework bundle.
    codesign --force --sign - "${VERSION_DIR}/${FRAMEWORK_NAME}" 2>&1 \
        | grep -v "replacing existing signature" || true
    codesign --force --sign - "${FRAMEWORK_DIR}" 2>&1 \
        | grep -v "replacing existing signature" || true

    XCARGS+=(-framework "$FRAMEWORK_DIR")
done

xcodebuild -create-xcframework "${XCARGS[@]}" -output "$XCFRAMEWORK_DIR"

echo "==> Copying Marginalia.swift into Generated/ and SwiftPM source tree"
cp "${GEN_DIR}/Marginalia.swift" "${OUT_DIR}/Marginalia.swift"
# The SwiftPM package (`apps/mac-gui/MarginaliaUI/Package.swift`) loads the
# bindings from `Sources/MarginaliaKit/Marginalia.swift`. Writing only to
# `Generated/` leaves SwiftPM stuck on the previous binding even after a
# fresh `build-rust-xcframework.sh` run — surfaces as "has no member
# 'prefetchNext'"-style errors when the UDL grew new methods.
SPM_BINDINGS="${REPO_ROOT}/apps/mac-gui/MarginaliaUI/Sources/MarginaliaKit/Marginalia.swift"
if [ -d "$(dirname "$SPM_BINDINGS")" ]; then
    cp "${GEN_DIR}/Marginalia.swift" "$SPM_BINDINGS"
fi

echo
echo "Done. Artifacts in ${OUT_DIR}:"
echo "  • ${XCFRAMEWORK_NAME}.xcframework"
echo "  • Marginalia.swift"
