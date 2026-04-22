// swift-tools-version:6.0
//
// Marginalia macOS SwiftUI UI.
//
// Two build modes:
//
// 1. **Mock** (default) — `swift build` produces `MarginaliaUIPreview`, an
//    executable wired to `MockHost`. Used for UX iteration, design review, and
//    the demo bundle (`make bundle-mock` / `build-app-bundle.sh --mock`).
//
// 2. **Live** — set `MARGINALIA_LIVE=1` before `swift build` to additionally
//    produce `MarginaliaUILive`, which links `MarginaliaKit.xcframework` (the
//    Rust runtime packaged by `scripts/build-rust-xcframework.sh`) and drives
//    the UI via `FFIHost`. Used for the real shipped app
//    (`make bundle-live` / `build-app-bundle.sh --live`).
//
//    Live builds are Apple Silicon only (arm64). MLX/Metal is required for
//    TTS and we don't ship a non-MLX macOS path; Intel Macs aren't a target.
//
// In live mode the library target picks up `-D MARGINALIA_FFI`, which is what
// gates `FFIHost` inside `RuntimeBridge.swift`. The generated bindings
// (`Marginalia.swift`) live under `Sources/MarginaliaKit/` and are .gitignored —
// the live build script populates that directory before running swift build.
//
// Keeping the mock target permanent (per user request) means design/demo
// builds never need the Rust toolchain or the xcframework.

import PackageDescription
import Foundation

// Live mode is enabled in two situations:
// 1. `MARGINALIA_LIVE=1` is set at build time (the `swift build` dev path and
//    the shell-driven bundle script).
// 2. The generated UniFFI bindings file exists on disk — which is the state
//    after `build-rust-xcframework.sh` runs. This lets the Xcode project
//    (Workstream H) consume the SwiftPM package with `MARGINALIA_FFI` active
//    without plumbing an env var through the Xcode build system.
let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bindingsFile = packageRoot
    .appendingPathComponent("Sources/MarginaliaKit/Marginalia.swift")
let bindingsPresent = FileManager.default.fileExists(atPath: bindingsFile.path)

let liveEnabled =
    ProcessInfo.processInfo.environment["MARGINALIA_LIVE"] == "1" || bindingsPresent

let liveSettings: [SwiftSetting] = liveEnabled ? [.define("MARGINALIA_FFI")] : []
let uiDependencies: [Target.Dependency] = liveEnabled ? ["MarginaliaKit"] : []

var targets: [Target] = [
    .target(
        name: "MarginaliaUI",
        dependencies: uiDependencies,
        path: "Sources/MarginaliaUI",
        resources: [
            // Variable .ttf fonts shipped with the library. Registered at
            // launch via `Fonts.registerBundled()`; the typography helpers
            // in DesignTokens.swift resolve names against these.
            .process("Resources"),
        ],
        swiftSettings: liveSettings
    ),
    .executableTarget(
        name: "MarginaliaUIPreview",
        dependencies: ["MarginaliaUI"],
        path: "Sources/MarginaliaUIPreview"
    ),
]

var products: [Product] = [
    .library(name: "MarginaliaUI", targets: ["MarginaliaUI"]),
    .executable(name: "MarginaliaUIPreview", targets: ["MarginaliaUIPreview"]),
]

if liveEnabled {
    targets.append(contentsOf: [
        .binaryTarget(
            name: "MarginaliaFFI",
            path: "../Generated/MarginaliaKit.xcframework"
        ),
        .target(
            // Swift wrapper around the UniFFI-generated bindings. Named
            // `MarginaliaKit` to mirror the xcframework and avoid colliding
            // with the `Marginalia` app target in the Xcode project (which
            // would cause duplicate .swiftmodule outputs at build time).
            name: "MarginaliaKit",
            dependencies: ["MarginaliaFFI"],
            path: "Sources/MarginaliaKit"
            // `MarginaliaFFI` is a dylib wrapped in a `.framework`. dyld
            // resolves its own dependencies (Accelerate, Metal, Speech,
            // AVFoundation, libc++, SystemConfiguration, etc.) at load
            // time — we don't need to re-declare them as linkerSettings
            // on the consuming Swift target.
        ),
        .executableTarget(
            name: "MarginaliaUILive",
            dependencies: ["MarginaliaUI", "MarginaliaKit"],
            path: "Sources/MarginaliaUILive",
            swiftSettings: [.define("MARGINALIA_FFI")],
            linkerSettings: [
                // Tell dyld to look inside the .app's Frameworks/ dir for
                // MarginaliaFFI.framework at runtime. Xcode sets this up
                // automatically for .app targets; SwiftPM doesn't. Without
                // it, the app launches and immediately fails with
                // "Library not loaded: @rpath/MarginaliaFFI.framework/...".
                .unsafeFlags([
                    "-Xlinker", "-rpath",
                    "-Xlinker", "@executable_path/../Frameworks",
                ]),
            ]
        ),
    ])
    products.append(contentsOf: [
        .library(name: "MarginaliaKit", targets: ["MarginaliaKit"]),
        .executable(name: "MarginaliaUILive", targets: ["MarginaliaUILive"]),
    ])
}

let package = Package(
    name: "MarginaliaUI",
    defaultLocalization: "it",  // fallback for Localizable.strings lookup
    platforms: [
        .macOS(.v14)  // SwiftUI features we rely on require macOS 14+.
    ],
    products: products,
    targets: targets
)
