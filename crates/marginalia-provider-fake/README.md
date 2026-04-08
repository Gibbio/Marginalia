# marginalia-provider-fake

Deterministic fake providers and in-memory repositories for the Marginalia Beta
engine.

This crate is meant for:

- engine tests
- development bootstrap before real adapters exist
- future smoke flows and host integration scaffolding

It currently provides:

- in-memory document, session, note, and rewrite repositories
- recording event publisher
- fake playback engine
- fake TTS
- fake command STT and dictation STT
- fake rewrite and summary generators
