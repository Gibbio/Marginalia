# marginalia-stt-apple

Apple native speech-to-text via `SFSpeechRecognizer`, used for BOTH voice
command recognition AND note dictation. macOS only.

## What it does

Captures the microphone via `cpal` on the Rust side, processes it through
**WebRTC AEC3** (via the `aec3` crate, pure Rust) to remove TTS playback
echo, and feeds the cleaned audio to a persistent Swift helper subprocess
that runs `SFSpeechRecognizer`. The helper no longer touches the mic itself
— it receives pre-processed PCM frames via a binary stdin protocol.

A single helper process handles both short-utterance command recognition and
long-form dictation, switching between the two modes on demand.

No models to download — uses the Neural Engine via the system dictation
framework. Requires macOS Dictation to be enabled
(`System Settings → Keyboard → Dictation → ON`).

## Usage

```rust
use marginalia_stt_apple::new_apple_stt;

let (recognizer, dict_transcriber, aec_pipeline) = new_apple_stt(
    "it-IT",                                  // BCP-47 language
    vec!["avanti".into(), "stop".into()],     // trigger words
    0.8,                                       // cmd_silence_timeout (s)
    1.5,                                       // dict_silence_timeout (s)
    60.0,                                      // dict_max_seconds
)?;

runtime.set_command_recognizer(recognizer);
runtime.set_dictation_transcriber(dict_transcriber);
```

The two returned handles share the same Swift helper via
`Arc<AppleHelperShared>`. Do not construct them separately — they must share
the process, the microphone stream, and the stdin mode-switch channel.

## Architecture

```
                   ┌──────────────────────────┐
                   │   Swift helper process   │
                   │   (SFSpeechRecognizer)   │
                   │                          │
                   │   MODE = command | dict  │
                   └───┬──────────────┬───────┘
                 stdin │              │ stdout
                       │              │
            MODE COMMAND│              │ CMD <text>    — commands
            MODE DICTATION│            │ DICT_END <text> — dictation
                       │              │
                ┌──────┴───┐  ┌───────┴──────┐
                │  stdin   │  │   reader     │
                │  Mutex   │  │   thread     │
                │ (shared) │  │  (routes by  │
                └────┬─────┘  │   prefix)    │
                     │        └──┬────────┬──┘
                     │           │        │
                     │      cmd_rx        dict_rx
                     │           │        │
                ┌────┴───────────┴───┐  ┌─┴─────────────────────┐
                │ AppleCommandRecognizer │  │ AppleDictationTranscriber │
                │   (CommandRecognizer)  │  │   (DictationTranscriber)  │
                └────────────────────┘  └─────────────────────┘
```

### Mode switching

The Swift helper reads lines from its stdin on a background queue:

- `MODE COMMAND` — switch to command recognition profile (short silence
  timeout, fast-path emit on trigger words, 1-shot utterances)
- `MODE DICTATION` — switch to dictation profile (long silence timeout, no
  trigger fast-path, accumulate full utterance then emit)

On every mode change, `scheduleRestart()` ends the current recognition task
and starts a fresh one after a ~300ms delay. The new task uses the current
mode's parameters.

Mode is read **live** inside the silence timer's `DispatchWorkItem` — so if
the user is mid-dictation when a mode flip happens, the timer will use the
current mode at fire-time, not a stale captured value.

### Line prefixes

The Swift helper prefixes every output line so the Rust reader thread can
route it to the right mpsc channel:

| Prefix      | Meaning                                    | Channel    |
|-------------|--------------------------------------------|------------|
| `CMD `      | Recognized utterance in command mode       | `cmd_rx`   |
| `DICT_END ` | Finalized dictation-mode utterance         | `dict_rx`  |

The reader thread is a single `std::thread` spawned by `new_apple_stt` that
reads the helper's stdout line-by-line and dispatches based on prefix.

### Shared state

```rust
pub struct AppleHelperShared {
    stdin: Mutex<ChildStdin>,   // serialized mode commands
    child: Mutex<Child>,        // kept alive; killed on last Arc drop
}
```

Both `AppleCommandRecognizer` and `AppleDictationTranscriber` hold an
`Arc<AppleHelperShared>`. When the last reference is dropped, `Drop` kills
the helper process and waits for it to exit.

### Dictation flow

1. Rust locks `dict_rx`, drains any stale lines
2. Rust writes `MODE DICTATION\n` to `stdin` (locked briefly)
3. Swift plays a Tink (start beep), switches mode, restarts recognition task
4. User talks; Swift accumulates partials
5. After `dict_silence_timeout` of silence, Swift emits `DICT_END <text>`
6. Rust receives the line, builds a `DictationTranscript`
7. Rust writes `MODE COMMAND\n` to `stdin` regardless of success
8. Swift plays a Pop (end beep) and switches back to command mode

If step 5 times out (`dict_max_seconds`), Rust still executes steps 6-7 with
an error text, ensuring the helper always returns to command mode.

### Trigger fast-path

In command mode, when a partial result already contains one of the configured
trigger words, the helper emits it immediately instead of waiting for the
silence timer. Cuts command latency from ~1.5s to ~200-300ms.

The fast-path is **disabled in dictation mode** — the user might legitimately
say "stop" or "ferma" inside a dictated note without wanting the session
stopped.

### Audio feedback

Dictation start and end are announced by system sounds played via
`AudioServicesPlaySystemSound` from the `AudioToolbox` framework. Loaded once
at helper startup from `/System/Library/Sounds/{Tink,Pop}.aiff`. Silent
fallback if loading fails. Triggered **only on actual mode transitions** (not
on redundant `MODE` writes to the same mode).

No AppKit / NSSound / NSApplication — pure AudioToolbox, no GUI runtime.

## Swift helper compilation

The Swift source is embedded in `lib.rs` as `SWIFT_HELPER_SOURCE`. At runtime,
`ensure_helper()` writes the source to a temp directory and invokes `swiftc`
to produce a binary at:

```
$TMPDIR/marginalia-stt-apple/stt-helper-v<HELPER_VERSION>
```

The first call per user session compiles the helper (~2s); subsequent calls
reuse the cached binary.

**`HELPER_VERSION` must be bumped whenever `SWIFT_HELPER_SOURCE` changes**,
otherwise users running a previous version of the binary will silently pick
up the old cached helper. The version is part of the binary path.

Required frameworks (passed via `-framework` to `swiftc`):

- `Speech` — `SFSpeechRecognizer`
- `AVFoundation` — `AVAudioEngine`, mic tap
- `AudioToolbox` — `AudioServicesPlaySystemSound` (dictation beeps)
- `Foundation` — stdlib

## Requirements

- macOS 11+ (recommended macOS 13+ for on-device recognition guarantees)
- Xcode Command Line Tools (`swiftc` on PATH)
- `System Settings → Keyboard → Dictation → ON`
- Microphone permission granted to the terminal/parent app

The helper errors with `"Siri and Dictation are disabled"` in stderr if
Dictation is not enabled — this is detected by `new_apple_stt` during its
0.5s smoke test and surfaced as a helpful error message.
