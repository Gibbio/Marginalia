# Reading State Machine

## States

- `IDLE`: no active reading session
- `READING`: session is actively reading the current document location; in Alpha 0.1 this is normally paired with `command_listening_active = true`
- `PAUSED`: playback is paused but the session remains active and the runtime may still be listening for commands
- `LISTENING_FOR_COMMAND`: reserved explicit capture state; it is no longer the primary Alpha 0.1 runtime mode
- `RECORDING_NOTE`: note capture is in progress
- `PROCESSING_REWRITE`: a rewrite request is running
- `READING_REWRITE`: reserved for playing back a rewrite draft
- `ERROR`: terminal fault state until explicitly reset

## Transition Summary

| From | To |
| --- | --- |
| `IDLE` | `READING`, `ERROR` |
| `READING` | `IDLE`, `PAUSED`, `LISTENING_FOR_COMMAND`, `RECORDING_NOTE`, `PROCESSING_REWRITE`, `ERROR` |
| `PAUSED` | `IDLE`, `READING`, `LISTENING_FOR_COMMAND`, `RECORDING_NOTE`, `PROCESSING_REWRITE`, `ERROR` |
| `LISTENING_FOR_COMMAND` | `IDLE`, `READING`, `PAUSED`, `RECORDING_NOTE`, `ERROR` |
| `RECORDING_NOTE` | `PAUSED`, `READING`, `ERROR` |
| `PROCESSING_REWRITE` | `READING_REWRITE`, `PAUSED`, `ERROR` |
| `READING_REWRITE` | `PAUSED`, `READING`, `ERROR` |
| `ERROR` | `IDLE` |

## Playback Projection

The high-level reader state maps to a projected playback state:

- `READING` and `READING_REWRITE` project to `playing`
- `IDLE` projects to `stopped`
- all other active workflow states project to `paused`

This matters because frontends may reconnect to a long-running backend process.
Playback state is therefore synchronized from persisted session metadata plus
the current playback adapter snapshot rather than from frontend-local state.

## Why Explicit State Matters

- voice-driven tools become confusing when note capture and playback overlap
- future TUI, desktop, editor, and mobile clients will need a single state
  vocabulary
- tests can assert lifecycle transitions before real speech providers exist

## Alpha 0.2 Implementation Notes

The repository implements the state graph and uses it in real CLI flows:

- `play`
- `pause`
- `resume`
- `repeat`
- `restart-chapter`
- `next-chapter`
- `stop`
- `note-start`
- `note-stop`
- `rewrite-current`

Implemented transition behavior:

- `play` ingests or selects a document, starts playback automatically, opens
  command listening automatically, and keeps both active until completion or
  explicit stop
- `pause` moves `READING -> PAUSED`
- `resume` moves `PAUSED -> READING`
- `stop` moves the active runtime to `IDLE` and stops both playback and the
  continuous command listener
- `note-start` moves `PAUSED` or `READING -> RECORDING_NOTE`
- `note-stop` persists a note and returns the session to `PAUSED`
- `rewrite-current` moves `PAUSED` or `READING -> PROCESSING_REWRITE -> PAUSED`
- `restart-chapter` and `next-chapter` update the persisted reading position
  and replay audio when the session is actively reading
- `repeat` re-synthesizes and replays the current reading chunk

`READING_REWRITE` remains intentionally defined but only partially exercised.
It stays reserved so later rewrite-playback work does not invent incompatible
state names.

### Step-Driven Runtime Model (Alpha 0.2)

As of Alpha 0.2 the read-while-listen runtime is driven by a `RuntimeLoop`
class that exposes a `step()` function returning a `StepStatus`:

- `CONTINUE` — the caller should invoke `step()` again
- `COMPLETED` — the document finished playing
- `STOPPED` — the session was stopped by a command or signal
- `ERROR` — a fatal runtime fault occurred

The caller owns the loop driver. The backend can run it in a worker thread; a
client may poll snapshots on a timer; an async wrapper may use `asyncio`. The
core does not assume any specific concurrency model.

### Voice Command Dispatch

Voice command dispatch uses a dict-driven table mapping `VoiceCommandIntent`
enum members to handler callables. Adding a new intent requires:

1. a new member in `VoiceCommandIntent`
2. at least one phrase in each language TOML file
3. a new entry in the dispatch table

Unhandled intents return an explicit error instead of silently falling through
to `stop`.

### Completion vs Stop Events

Document completion emits a `READING_COMPLETED` event and sets
`runtime_status = "completed"` with `last_command = "document-complete"`.
Explicit stop sets `runtime_status = "stopped"` with `last_command = "stop"`.
This distinction is tested and can be relied upon by future frontend clients.
