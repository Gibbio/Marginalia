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

## Why Explicit State Matters

- voice-driven tools become confusing when note capture and playback overlap
- future desktop and API clients will need a single state vocabulary
- tests can assert lifecycle transitions before real speech providers exist

## Bootstrap Implementation Notes

The repository currently implements the state graph and exercises the most useful
local transitions:

- `play`
- `pause`
- `resume`
- `note-start`
- `note-stop`
- `restart-chapter`
- `next-chapter`

`LISTENING_FOR_COMMAND` and `READING_REWRITE` are defined now so future provider
work does not invent incompatible state names later.
