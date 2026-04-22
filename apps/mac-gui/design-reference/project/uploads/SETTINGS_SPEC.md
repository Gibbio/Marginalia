# Marginalia — Settings Page Design Spec

Handoff document for the macOS Settings page. Pair with
`marginalia.reference.toml` (same directory) for the full list of options.

## Reading order

The Settings page is a vertical stack of sections. Suggested order,
top to bottom:

1. **Language** — one control, drives defaults for Voice + STT.
2. **Voice** — voice picker (depends on Language).
3. **Speech recognition** — STT engine + per-context tuning.
4. **Voice commands** — editable trigger→action table.
5. **Audio & reading** — chunk size, cache directory.
6. **Installations** — what's on disk, what to download.
7. **About / Diagnostics** — collapsed; versions, provider doctor blobs.

## What's static vs dynamic

**Static** (from `marginalia.toml`): user's current selections.
**Dynamic** (from `discovery.*()`): the lists that populate pickers.

The dynamic lists change whenever the user installs or removes an asset.
Re-fetch on Settings page open and after every install/uninstall.

```swift
// Populate pickers
let langs    = runtime.listLanguages()            // [LangInfo]
let voices   = runtime.listVoices(backend: "mlx") // [VoiceInfo]
let sttEng   = runtime.listSttEngines()           // [SttEngine]
let ttsBack  = runtime.listTtsBackends()          // [TtsBackend]

// Read current selection
let current  = runtime.currentSpec()              // ProviderSpec
```

## The Apply button — `ProviderSpec` → `ApplyReport`

Settings edits are staged locally in SwiftUI state. The user taps **Apply**
(disabled until there are actual changes). The button calls a single FFI:

```swift
let spec = ProviderSpec(
    ttsBackend: "mlx",
    voice:      "if_sara",
    sttEngine:  "apple",
    language:   "it-IT"
)

do {
    let report = try await Task.detached {
        try runtime.applyProviderSpec(spec: spec)
    }.value
    // report.ttsSwapped, report.sttSwapped, report.languageChanged, report.elapsedMs
} catch FfiError.Reconfigure(let msg) {
    // show inline error
}
```

Typical latencies:
| Change                        | Cost        | UX                        |
|-------------------------------|-------------|---------------------------|
| voice only                    | <50 ms      | no spinner                |
| TTS backend                   | 100–500 ms  | brief spinner             |
| STT engine or language        | 300–800 ms  | modal overlay "Cambio…"   |

Show the modal overlay only when `stt_swapped || language_changed` are about
to be true (compare spec to `currentSpec()` before calling).

## `VoiceInfo` — what the Voice picker gets

```swift
struct VoiceInfo {
    let id:       String   // "if_sara"                — stable identifier
    let display:  String   // "Sara"                   — show this to user
    let lang:     String   // "it-IT"                  — BCP-47
    let gender:   Gender   // .female | .male | .unknown
    let backend:  String   // "mlx"
}
```

Group voices by `lang`; within a group, sort by `gender` then `display`.
Show gender as an icon (figure.stand / figure.stand.dress or similar).

## `TtsBackend` / `SttEngine` — with availability

```swift
struct TtsBackend {
    let id:         String    // "mlx"
    let name:       String    // "Kokoro (MLX, Apple Silicon)"
    let available:  Bool
    let reason:     String?   // human-readable when !available
}
```

When `!available`, the row should be visibly disabled (greyed) AND show the
`reason` string. If the reason is actionable ("weights missing at …"),
pair it with an **Install** button that kicks the first-run downloader.

## Installations section

Separate scroll region at the bottom of Settings. Each row:

| Asset | Size | Status | Action |
|---|---|---|---|
| Kokoro MLX (Italian voices) | 74 MB | ✓ Installed | — |
| Whisper small (multilingual STT) | 465 MB | ✗ Not installed | [Install] |
| Voice: Sara (Italian female) | 0.5 MB | ✓ Installed | [Remove] |
| Voice: Nicola (Italian male) | 0.5 MB | ✓ Installed | [Remove] |
| Voice: Bella (English female)   | 0.5 MB | ✗ Not installed | [Install] |
| ONNX Runtime (TTS fallback)     | 34 MB  | ✗ Not installed | [Install] |
| PDFium (PDF import)             | 68 MB  | ✗ Not installed | [Install] |

**This is the only place in the app that is allowed to make network calls.**
Show a banner at the top of the section:

> Marginalia runs fully offline except for this page. Clicking **Install** will
> download from huggingface.co / github.com.

## Language selector — behavior

Placed at the very top. Changing the language should:

1. Immediately filter the Voice picker to voices matching the new BCP-47 tag.
2. If the currently-selected voice doesn't match the new language, pick the
   first voice in the new filtered list as the proposed value.
3. Queue a language change in the staged ProviderSpec (Apply button lights up).

If the language has zero installed voices, show a hint card:
> No voices installed for English. [Install English voices…]

## STT engine — per-language availability

Both Apple and Whisper are multilingual. The engine row doesn't gate per
language — just show engine availability from `listSttEngines()`. The UI
should, however, note:
- Apple STT requires **System Settings → Keyboard → Dictation = ON**.
  If the Swift helper smoke-test fails with that message, `apply_provider_spec`
  returns `FfiError.Reconfigure("Apple STT requires macOS Dictation…")` — the
  UI should translate this into a call-to-action with a "Open System Settings"
  button that runs `NSWorkspace.shared.open(URL(string: "x-apple.systempreferences:com.apple.preference.keyboard")!)`.

## Voice commands editor

Eleven fixed actions. User edits the list of trigger words per row. Validation:
- Words must be non-empty.
- No duplicate word across rows (would make `resolve_action` ambiguous).
- Accept any language (the STT recognizes raw audio; the word just needs to
  be something the user will actually say).

The edit is a TOML write — not an `apply_provider_spec` call. Writing the
file is enough; the next capture cycle picks up the new triggers.

## Error surfaces

All FFI calls can throw `FfiError`. Variants:
- `.config(String)` — malformed marginalia.toml; show in a banner at the top
  of Settings, with a "Revert to defaults" button.
- `.io(String)` — filesystem failure; typically "couldn't write settings".
- `.build(String)` — runtime init failed; fatal, the app can't start.
- `.reconfigure(String)` — apply_provider_spec failed; inline toast, leave the
  staged spec intact so the user can retry.

## Diagnostics panel (collapsed by default)

Read-only:
- TTS provider label (e.g. "kokoro-mlx")
- STT engine label
- Model paths
- Cache directory + current size on disk
- Last `apply_provider_spec` ApplyReport

Useful for support conversations; also shows users *why* a backend is unavailable.

## Accessibility

- All dropdowns full keyboard-accessible.
- The Voice picker should announce `display + gender + lang` for VoiceOver.
- Language names are in the target language (italiano, English, 日本語) — use
  the `display` field from `LangInfo` as-is.

## Anti-patterns (please avoid)

- **Don't duplicate the voice list into marginalia.toml.** It is computed at
  runtime; a static list rots the moment the user adds/removes a voice.
- **Don't add a "network access" master switch.** The app is offline-only by
  construction; only the Installations section makes network calls, gated
  by explicit user action. A global toggle would be misleading.
- **Don't auto-apply on every change.** Staging + an explicit Apply button
  lets the user change language + voice + STT together in a single respawn
  instead of three sequential ones (spec diff collapses all of them).
- **Don't restart the app on reconfigure.** `apply_provider_spec` is designed
  to hot-swap in <1 s. Using "Quit and relaunch" is a UX regression.
