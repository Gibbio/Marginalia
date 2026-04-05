# Reading State Machine

## States

- `IDLE`: no active reading session
- `READING`: session is actively reading the current document location
- `PAUSED`: playback is paused but the session remains active
- `LISTENING_FOR_COMMAND`: reserved for future spoken command capture
- `RECORDING_NOTE`: note capture is in progress
- `PROCESSING_REWRITE`: a rewrite request is running
- `READING_REWRITE`: reserved for playing back a rewrite draft
- `ERROR`: terminal fault state until explicitly reset

## Transition Summary

| From | To |
| --- | --- |
| `IDLE` | `READING`, `ERROR` |
| `READING` | `PAUSED`, `LISTENING_FOR_COMMAND`, `RECORDING_NOTE`, `PROCESSING_REWRITE`, `ERROR` |
| `PAUSED` | `READING`, `LISTENING_FOR_COMMAND`, `RECORDING_NOTE`, `PROCESSING_REWRITE`, `ERROR` |
| `LISTENING_FOR_COMMAND` | `READING`, `PAUSED`, `RECORDING_NOTE`, `ERROR` |
| `RECORDING_NOTE` | `PAUSED`, `READING`, `ERROR` |
| `PROCESSING_REWRITE` | `READING_REWRITE`, `PAUSED`, `ERROR` |
| `READING_REWRITE` | `PAUSED`, `READING`, `ERROR` |
| `ERROR` | `IDLE` |

## Playback Projection

The high-level reader state maps to a projected playback state:

- `READING` and `READING_REWRITE` project to `playing`
- `IDLE` projects to `stopped`
- all other active workflow states project to `paused`

This matters because the CLI is currently a one-shot process model. The fake
playback engine is re-created on each invocation, so the persisted session state
remains the source of truth for coherent status reporting.

## Why Explicit State Matters

- voice-driven tools become confusing when note capture and playback overlap
- future desktop and API clients will need a single state vocabulary
- tests can assert lifecycle transitions before real speech providers exist

## V0 Implementation Notes

The repository currently implements the state graph and uses it in real CLI
flows:

- `play`
- `pause`
- `resume`
- `repeat`
- `restart-chapter`
- `next-chapter`
- `note-start`
- `note-stop`
- `rewrite-current`

Implemented transition behavior:

- `play` creates a session for the explicitly requested document, the active
  session document, or the latest ingested document
- `pause` moves `READING -> PAUSED`
- `resume` moves `PAUSED -> READING`
- `note-start` moves `PAUSED` or `READING -> RECORDING_NOTE`
- `note-stop` persists a note and returns the session to `PAUSED`
- `rewrite-current` moves `PAUSED` or `READING -> PROCESSING_REWRITE -> PAUSED`
- `restart-chapter` and `next-chapter` update the persisted reading position
  without inventing new lifecycle states
- `repeat` is a read-only query against the current persisted position

`LISTENING_FOR_COMMAND` and `READING_REWRITE` remain intentionally defined but
only partially exercised. They are reserved so later voice-command and
rewrite-playback work does not invent incompatible state names.
