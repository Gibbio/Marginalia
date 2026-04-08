# Beta Repository Structure

## Intent

The Beta repository is organized around a shared engine and host-specific app
shells.

The important distinction is:

- `crates/` owns shared engine code
- `apps/` owns host applications and OS integration
- `models/` owns local AI artifacts and packaging layout

## Target Top-Level Map

### `crates/`

Shared engine crates:

- `marginalia-core`: domain, application services, contracts, events, ports
- `marginalia-runtime`: engine composition and host-neutral lifecycle
- `marginalia-storage-sqlite`: SQLite persistence and migrations
- `marginalia-provider-fake`: fake adapters for testing and development
- `marginalia-tts-*`: text-to-speech providers
- `marginalia-stt-*`: speech-to-text providers
- `marginalia-playback-host`: playback bridge contracts and host adapters
- `marginalia-ffi`: bindings and API boundary for host shells
- `marginalia-devtools`: doctor, smoke, benchmark, and migration helpers

### `apps/`

Host applications:

- `desktop/`: desktop reference host
- `ios/`: iOS shell
- `android/`: Android shell

### `models/`

Local model assets organized by capability:

- `tts/kokoro/`
- `stt/vosk/`
- `stt/whisper/`
- `llm/`

### `docs/`

Architecture, ADRs, migration records, product plans, and roadmap.

### `tests/`

Cross-crate engine tests, host integration tests, and shared fixtures.

## Transitional Rule

Alpha Python code remains in the repository during migration, but it should no
longer define the target Beta structure.

The Beta tree is introduced first so that implementation work can move into the
correct ownership boundaries incrementally.
