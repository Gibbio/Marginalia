# Frontend Client Guide

## Purpose

This document explains how to build a frontend client against the new
Marginalia backend.

It is written for developers building:

- a Rust TUI
- a desktop GUI
- an Obsidian plugin
- any other local client

The goal is to make client work straightforward without exposing internal
backend services or storage details.

## Current Shape

Today the backend is exposed as a local process that serves a versioned
frontend contract over `stdio` using JSON Lines.

The contract boundary lives in:

- [`packages/core/src/marginalia_core/application/frontend`](/home/debian/sources/Marginalia/packages/core/src/marginalia_core/application/frontend)

The current backend entrypoint lives in:

- [`apps/backend/src/marginalia_backend/main.py`](/home/debian/sources/Marginalia/apps/backend/src/marginalia_backend/main.py)

The current stdio transport lives in:

- [`apps/backend/src/marginalia_backend/stdio_server.py`](/home/debian/sources/Marginalia/apps/backend/src/marginalia_backend/stdio_server.py)

## Backend Lifecycle

### Start the backend

Run the backend as a child process:

```bash
python -m marginalia_backend serve-stdio
```

Or with the console script:

```bash
marginalia-backend serve-stdio
```

If you use a config file:

```bash
marginalia-backend serve-stdio --config marginalia.toml
```

### Process model

The frontend owns the backend child process lifecycle.

That means:

- spawn the backend when the frontend starts
- keep stdin/stdout pipes open
- kill or clean up the child process on frontend exit
- treat EOF on stdout as backend termination

## Transport

### Wire format

Transport is newline-delimited JSON:

- one JSON object per line
- frontend writes requests to backend stdin
- backend writes responses to stdout
- each request gets exactly one response

Do not write frontend debug logs into the same stdout channel if you are
wrapping or proxying the backend process. Keep protocol I/O clean.

### Envelope

Every request includes:

- `type`: `"command"` or `"query"`
- `name`: contract operation name
- `payload`: object
- `id`: client-generated request id
- `protocol_version`: currently `1`

Every response includes:

- `status`: `"ok"` or `"error"`
- `name`: echoed operation name
- `message`: human-readable result text
- `payload`: object
- `request_id`: echoed request id when available
- `protocol_version`: currently `1`

## Minimal Request Example

```json
{"type":"query","name":"get_backend_capabilities","payload":{},"id":"req-1","protocol_version":1}
```

Example response:

```json
{"status":"ok","name":"get_backend_capabilities","message":"Backend capabilities reported.","payload":{"protocol_version":1,"commands":["create_note","ingest_document","next_chapter","next_chunk","pause_session","previous_chapter","previous_chunk","repeat_chunk","restart_chapter","resume_session","start_session","stop_session"],"queries":["get_app_snapshot","get_backend_capabilities","get_document_view","get_doctor_report","get_session_snapshot","list_notes","list_documents","search_documents","search_notes"],"transports":["stdio-jsonl"],"frontend_event_stream_supported":true,"dictation_enabled":true,"rewrite_enabled":true,"summary_enabled":true},"request_id":"req-1","protocol_version":1}
```

## First Handshake

Every new client should do this first:

1. spawn backend
2. send `get_backend_capabilities`
3. verify `protocol_version`
4. cache the command/query lists
5. degrade gracefully if a feature is absent

This avoids hard-coding assumptions into the frontend.

## Commands

Current command names are defined in:

- [`packages/core/src/marginalia_core/application/frontend/commands.py`](/home/debian/sources/Marginalia/packages/core/src/marginalia_core/application/frontend/commands.py)

Currently supported commands:

- `create_note`
- `ingest_document`
- `next_chapter`
- `next_chunk`
- `pause_session`
- `previous_chapter`
- `previous_chunk`
- `repeat_chunk`
- `restart_chapter`
- `resume_session`
- `start_session`
- `stop_session`

### Command payloads

#### `start_session`

Payload:

```json
{"target":"tests/fixtures/sample_document.txt"}
```

`target` may be:

- a filesystem path
- a stored document id
- omitted or empty, if the backend supports resolving the current default

#### `ingest_document`

Payload:

```json
{"path":"tests/fixtures/sample_document.txt"}
```

Client note:

- the backend accepts plain text and markdown files
- frontend clients may expand shell-like paths such as `~/...`, `$HOME/...`,
  and `${HOME}/...` before sending the request
- in the current Rust TUI, `ingest` is used to import a document into the local
  library and immediately show it in the document preview, even before `play`

#### `create_note`

Payload:

```json
{"text":"Important passage to revisit later."}
```

#### Navigation / playback commands

These use an empty payload:

- `pause_session`
- `resume_session`
- `stop_session`
- `repeat_chunk`
- `restart_chapter`
- `previous_chapter`
- `previous_chunk`
- `next_chunk`
- `next_chapter`

Frontend note:

- the current Rust TUI binds these commands to arrow keys when the command bar
  is empty
- `Up` = `previous_chunk`
- `Down` = `next_chunk`
- `Left` = `previous_chapter`
- `Right` = `next_chapter`
- when the command bar is not empty, `Up` and `Down` remain bound to command
  suggestion navigation instead

## Search Queries

The search queries share the same payload shape:

```json
{"query":"attentive"}
```

Supported queries:

- `search_documents`
- `search_notes`

The backend currently rejects empty or whitespace-only queries with an error
response.

## Queries

Current query names are defined in:

- [`packages/core/src/marginalia_core/application/frontend/queries.py`](/home/debian/sources/Marginalia/packages/core/src/marginalia_core/application/frontend/queries.py)

Currently supported queries:

- `get_app_snapshot`
- `get_backend_capabilities`
- `get_document_view`
- `get_doctor_report`
- `get_session_snapshot`
- `list_notes`
- `list_documents`
- `search_documents`
- `search_notes`

### Recommended polling model

The stdio transport is currently request/response only.

That means a frontend should:

- query `get_app_snapshot` on startup
- query `list_documents` on startup
- query `get_session_snapshot` after every successful command
- optionally poll snapshots on a timer while a session is active

For example:

- idle UI: poll slowly or not at all
- active reading session: poll every 500-1000 ms

## Snapshot Shapes

### `get_app_snapshot`

Returns:

- backend state
- active session id
- document count
- latest document id
- playback state
- runtime status

This is the high-level boot snapshot for a client.

### `get_session_snapshot`

Returns `null` if no session is active.

Otherwise it returns a stable projection with:

- session id
- document id
- current section and chunk
- current chunk text
- reading state
- playback state
- provider names
- note count
- anchor

Use this snapshot to drive the main reading UI.

### `list_documents`

Returns document summaries:

- `document_id`
- `title`
- `chapter_count`
- `chunk_count`

Use this to render pickers, command palettes, or sidebars.

## Error Handling

A response with `status="error"` is part of the protocol, not a transport
failure.

Treat the two failure classes separately:

### Protocol-level error

Examples:

- unknown command
- missing required payload field
- backend rejected operation because no session is active

Handling:

- show the backend `message`
- keep the client alive
- refresh snapshots if useful

### Transport/process error

Examples:

- backend child process exits
- stdout closes unexpectedly
- response line is not valid JSON

Handling:

- mark backend as disconnected
- stop sending further requests
- offer reconnect or restart

## State Ownership

The backend is authoritative.

The frontend must not invent its own long-lived source of truth for:

- reading state
- playback state
- note state
- document persistence

The correct frontend model is:

- render from snapshots
- trigger mutations via commands
- refresh after mutations

## What Not To Do

- do not import Python services directly from a frontend
- do not inspect SQLite files from the frontend
- do not assume a document is active without checking snapshots
- do not rely on log output as protocol output
- do not treat backend response messages as stable parsing targets

Parse only structured JSON fields.

## Current Limitation

The stdio backend does not yet expose a live event stream over the same channel.

The contract includes frontend event types conceptually, but the current client
implementation should still rely on:

- synchronous command responses
- explicit snapshot queries
- optional polling

When event streaming is added later, clients should keep the same command/query
model and only replace part of their refresh logic.

## Suggested Client Architecture

A good frontend client should split into:

### Transport adapter

Responsible for:

- spawning backend
- writing request lines
- reading response lines
- request ids
- process cleanup

### Contract client

Responsible for:

- typed command helpers
- typed query helpers
- JSON decode into client DTOs
- protocol version checks

### UI layer

Responsible for:

- rendering
- input
- local history
- keybindings
- focus state

The UI layer should not know how the backend process is launched.

## Suggested Build Order For A New Client

1. Implement backend spawn and cleanup.
2. Implement `get_backend_capabilities`.
3. Implement `get_app_snapshot`.
4. Implement `list_documents`.
5. Implement `start_session` and `stop_session`.
6. Implement `get_session_snapshot`.
7. Add note and navigation commands.
8. Add polling only after command/query basics are stable.

## Reference

For the architectural rationale behind this boundary, see:

- [`docs/architecture/frontend-backend-boundary.md`](/home/debian/sources/Marginalia/docs/architecture/frontend-backend-boundary.md)
