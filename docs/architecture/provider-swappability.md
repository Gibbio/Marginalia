# Provider Swappability

## Goal

Marginalia must not lock its core workflow to one speech or model provider. The
product depends on replaceability because local AI tooling evolves quickly and
user constraints will vary.

## Port Boundaries

Current ports cover:

- command STT recognizer
- dictation STT transcriber
- speech synthesizer
- playback engine
- rewrite generator
- topic summarizer
- storage repositories
- event publisher and subscriber

## Rules

- the core depends only on ports
- real providers live in adapters or infra, never in the core
- CLI code composes concrete implementations but does not become business logic
- future providers may be local, hybrid, or remote, but the domain model stays
  unchanged

## Why This Matters Early

If provider swappability is delayed, fake adapters quickly harden into hidden
product assumptions. By defining ports now, the repository can support later
experiments with:

- local whisper-style transcription
- local or OS-native TTS
- higher-quality playback stacks
- different rewrite and summarization backends

without reworking the core application services.

## Deferred Work

Provider quality, latency, device access, and model lifecycle management are all
future concerns. This repository only establishes the shape required to support
them safely later.
