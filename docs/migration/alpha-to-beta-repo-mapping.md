# Alpha to Beta Repository Mapping

## Purpose

This document maps the current Alpha Python repository to the target Beta
repository shape.

The Beta target is:

- one shared engine
- native or host-specific shells
- portable local AI runtime decisions
- explicit separation between engine and host

The current Beta branch does not migrate all code immediately. It introduces
the target shape first, then moves implementation slices into it.

## Target Beta Top Level

```text
Marginalia/
├── apps/
│   ├── android/
│   ├── desktop/
│   └── ios/
├── crates/
├── docs/
├── models/
│   ├── llm/
│   ├── stt/
│   └── tts/
├── scripts/
└── tests/
```

`models/` is organized by capability, not by individual model name. That is
why Kokoro should live under `models/tts/kokoro/`, not directly under
`models/kokoro/`.

## Mapping Rules

- shared product semantics move into `crates/`
- host-specific UI and OS integration move into `apps/`
- model artifacts move into `models/`
- Alpha Python code remains in place until the corresponding Beta destination
  exists and is ready to take ownership

## Alpha to Beta Mapping

### Core product model and use cases

Current:

- `packages/core/src/marginalia_core/domain/`
- `packages/core/src/marginalia_core/application/`
- `packages/core/src/marginalia_core/events/`
- `packages/core/src/marginalia_core/ports/`

Beta destination:

- `crates/marginalia-core/src/domain/`
- `crates/marginalia-core/src/application/`
- `crates/marginalia-core/src/events/`
- `crates/marginalia-core/src/ports/`
- `crates/marginalia-core/src/frontend/`

Notes:

- this is the highest-value migration area
- domain semantics should be preserved even if APIs change language
- frontend DTOs and contracts remain part of the shared engine boundary

### Runtime composition and host-neutral orchestration

Current:

- `apps/backend/src/marginalia_backend/bootstrap.py`
- `apps/backend/src/marginalia_backend/runtime.py`
- `packages/core/src/marginalia_core/application/services/runtime_loop.py`
- `packages/core/src/marginalia_core/application/services/reading_runtime_service.py`

Beta destination:

- `crates/marginalia-runtime/src/bootstrap.rs`
- `crates/marginalia-runtime/src/container.rs`
- `crates/marginalia-runtime/src/lifecycle.rs`
- `crates/marginalia-core/src/application/services/runtime_loop.rs`

Notes:

- engine orchestration belongs in shared crates
- process-level hosting does not

### Frontend contract and snapshots

Current:

- `packages/core/src/marginalia_core/application/frontend/`
- `apps/backend/src/marginalia_backend/gateway.py`

Beta destination:

- `crates/marginalia-core/src/frontend/`
- `crates/marginalia-ffi/src/api/`

Notes:

- contract types stay canonical
- transport and binding details move to FFI or host bridges

### SQLite persistence

Current:

- `packages/infra/src/marginalia_infra/storage/sqlite.py`
- `packages/infra/src/marginalia_infra/storage/migrations/`

Beta destination:

- `crates/marginalia-storage-sqlite/src/`
- `crates/marginalia-storage-sqlite/migrations/`

Notes:

- SQLite remains the first local store
- migrations remain explicit and versioned

### TTS providers

Current:

- `packages/adapters/src/marginalia_adapters/real/kokoro.py`
- `packages/adapters/src/marginalia_adapters/real/kokoro_worker.py`
- `packages/adapters/src/marginalia_adapters/real/piper.py`
- `packages/adapters/src/marginalia_adapters/fake/tts.py`

Beta destination:

- `crates/marginalia-tts-kokoro/src/`
- `crates/marginalia-tts-piper/src/`
- `crates/marginalia-provider-fake/src/tts.rs`
- `models/tts/kokoro/`

Notes:

- Kokoro remains the canonical TTS model family
- ONNX Runtime becomes the target runtime direction
- the Python worker is a temporary Alpha bridge, not the Beta destination

### STT providers

Current:

- `packages/adapters/src/marginalia_adapters/real/vosk.py`
- `packages/adapters/src/marginalia_adapters/real/whisper_cpp.py`
- `packages/adapters/src/marginalia_adapters/fake/stt.py`

Beta destination:

- `crates/marginalia-stt-vosk/src/`
- `crates/marginalia-stt-whisper/src/`
- `crates/marginalia-provider-fake/src/stt.rs`
- `models/stt/vosk/`
- `models/stt/whisper/`

Notes:

- command STT and dictation STT stay distinct responsibilities
- host-specific audio capture may remain outside the core recognizer logic

### Playback

Current:

- `packages/adapters/src/marginalia_adapters/real/playback.py`
- `packages/adapters/src/marginalia_adapters/fake/playback.py`

Beta destination:

- `crates/marginalia-playback-host/src/`
- `crates/marginalia-provider-fake/src/playback.rs`
- host-specific implementations in:
  - `apps/desktop/`
  - `apps/ios/`
  - `apps/android/`

Notes:

- playback semantics remain shared through ports
- OS audio session behavior should move out of Alpha subprocess assumptions

### LLM-related providers

Current:

- `packages/adapters/src/marginalia_adapters/fake/llm.py`

Beta destination:

- `crates/marginalia-llm-fake/src/`
- future real providers in separate `marginalia-llm-*` crates
- model assets under `models/llm/`

Longer-term reserved destination:

- `crates/marginalia-secrets/src/` for secure storage of API keys, OAuth
  tokens, refresh tokens, and other provider credentials through host-native
  secret stores

### Backend process host

Current:

- `apps/backend/`

Beta destination:

- temporary Alpha-compatible host retained during migration
- long-term host logic split between:
  - `crates/marginalia-runtime/`
  - `crates/marginalia-ffi/`
  - `apps/desktop/`
  - `apps/ios/`
  - `apps/android/`

Notes:

- the current backend process is a host implementation, not the engine

### CLI and TUI surfaces

Current:

- `apps/cli/`
- `apps/tui-rs/`

Beta destination:

- desktop shell responsibilities gradually converge into `apps/desktop/`
- `apps/tui-rs/` is retained as a Rust desktop development and administration
  tool during Beta
- development/admin utilities that are not interactive UIs move into
  `crates/marginalia-devtools/`

Notes:

- not every Alpha UI surface needs a direct Beta equivalent
- `tui-rs` is explicitly one of the tools we keep
- useful inspection tooling should survive, but not dictate the product shape

### Infra utilities

Current:

- `packages/infra/src/marginalia_infra/config/`
- `packages/infra/src/marginalia_infra/logging/`
- `packages/infra/src/marginalia_infra/events.py`
- `packages/infra/src/marginalia_infra/runtime/session_supervisor.py`
- `packages/infra/src/marginalia_infra/storage/cache.py`

Beta destination:

- host-neutral pieces move into `crates/marginalia-runtime/`
- storage pieces move into `crates/marginalia-storage-sqlite/`
- diagnostics and smoke tooling move into `crates/marginalia-devtools/`
- process and OS-specific supervision moves into host layers

## Recommended Migration Order

1. preserve and port core domain models
2. preserve and port contracts and DTOs
3. port SQLite repositories
4. port runtime orchestration
5. replace Alpha TTS path with Beta Kokoro runtime path
6. rebuild host shells around the shared engine

## Immediate Beta Scaffold

The Beta branch should now create these repository anchors even before all code
is migrated:

- `crates/`
- `apps/ios/`
- `apps/android/`
- `models/tts/kokoro/`
- `models/stt/vosk/`
- `models/stt/whisper/`
- `models/llm/`

This makes the future ownership boundaries visible in the repository now,
instead of leaving them implicit in planning documents only.
