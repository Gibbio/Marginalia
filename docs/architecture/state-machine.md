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

This matters because the CLI is currently a one-shot process model. Playback
state is therefore synchronized from persisted session metadata plus the current
playback adapter snapshot on each invocation.

## Why Explicit State Matters

- voice-driven tools become confusing when note capture and playback overlap
- future desktop and API clients will need a single state vocabulary
- tests can assert lifecycle transitions before real speech providers exist

## Alpha 0.1 Implementation Notes

The repository currently implements the state graph and uses it in real CLI
flows:

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
