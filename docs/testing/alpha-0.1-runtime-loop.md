# Alpha 0.1 Runtime Loop Verification

## Goal

Verify the only supported Alpha 0.1 runtime mode:

1. provide a file to `play`
2. playback starts automatically
3. microphone listening starts automatically
4. command listening stays active while playback is running
5. spoken commands are handled during playback
6. the session ends on document completion or `stop`
7. a new `play` cleans up any stale previous Marginalia runtime first

## Prerequisites

- macOS with `afplay`
- a working Kokoro or Piper setup if you want real TTS
- a working Vosk model plus `vosk` and `sounddevice` if you want real command STT
- microphone permission for the terminal app
- `doctor --json` showing `provider_checks.vosk.ready = true` and `provider_checks.playback.ready = true`

## Recommended Config

Use [examples/alpha-local-config.toml](/Users/mauriziogobbo/Marginalia/examples/alpha-local-config.toml).

Important checks:

- `command_language = "it"`
- default OS microphone is the one you want to use
- default OS output is the device you want to listen on
- `providers.allow_fallback = false` for honest real-provider runs

## Commands

Check readiness:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml doctor --json
```

Start the runtime with a file:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml play examples/voice-test-it.txt --json
```

Check status from another terminal while `play` is still running:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml status --json
```

Manual control from another terminal:

```bash
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml pause --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml resume --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml next-chapter --json
.venv/bin/python -m marginalia_cli --config examples/alpha-local-config.toml stop --json
```

## What To Verify

During `play`, confirm:

- audio starts without a separate `listen` step
- `status` shows `runtime.command_listening_active = true`
- `status` shows the expected `command_language`
- `status` shows the active providers and default audio devices
- spoken commands like `pausa`, `continua`, `ripeti`, `capitolo successivo`, `ricomincia capitolo`, `stato`, and `stop` work while playback is ongoing
- after the last chunk, the session moves to `IDLE` with `runtime_status = "completed"`
- after `stop`, the session moves to `IDLE` with `runtime_status = "stopped"`

## Restart Safety Check

1. Start `play` in one terminal.
2. Without stopping it cleanly, launch another `play` in a second terminal.
3. Confirm the new invocation reports startup cleanup.
4. Confirm only the new runtime remains active.
5. Confirm `status` reflects the new runtime pid and the latest cleanup summary.

## Notes

- Alpha 0.1 intentionally uses the OS default audio devices as the primary path.
- Simultaneous playback plus microphone capture is the supported runtime behavior.
- This guide does not validate note dictation, rewrite quality, or summary quality.
