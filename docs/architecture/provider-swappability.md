# Provider Swappability

## Goal

Marginalia must not lock its core workflow to one speech or model provider. The
product depends on replaceability because local AI tooling evolves quickly and
user constraints will vary.

## Port Boundaries

Current ports intentionally separate:

- command STT recognizer
- dictation STT transcriber
- speech synthesizer
- playback engine
- rewrite generator
- topic summarizer
- storage repositories
- event publisher and subscriber

Command STT and dictation STT are not treated as one generic voice interface.
They have different latency, transcript-shape, and future streaming needs.

## Capability Model

Providers now expose explicit capabilities so the core does not need to infer
behavior from provider names later.

Current capability fields include:

- provider name
- interface kind
- supported languages
- supports streaming
- supports partial results
- supports timestamps
- low-latency suitability
- offline capability
- execution mode: local, hybrid, or remote

This is intentionally small but realistic enough for future provider selection
without changing core service contracts.

## Structured Provider Results

Ports no longer return only primitive strings or bytes.

Current structured outputs include:

- `CommandRecognition` for command STT
- `DictationTranscript` and `DictationSegment` for dictation STT
- `SynthesisRequest` and `SynthesisResult` for TTS
- `PlaybackSnapshot` for playback state, provider name, and subprocess metadata
- `RewriteInstruction` and `RewriteOutput` for rewrite generation
- `SummaryInstruction` and `SummaryOutput` for summarization

This keeps future provider-specific richness behind port-shaped models rather
than leaking SDK details into domain services.

## Current Fake Providers

The repository intentionally ships deterministic development adapters:

- command STT returns scripted commands
- dictation STT returns stable transcripts with timestamp-like segments
- TTS returns stable synthesis metadata and fake audio references
- playback returns snapshots and deterministic state transitions
- rewrite returns explicit section-aware drafts
- summary returns structured local summaries with highlights

They are not mocks hidden inside tests. They are first-class development
adapters meant to exercise the architecture honestly.

## Current Real Local Providers

Alpha 0.1 adds a narrow real local path without changing the port shape:

- Piper CLI synthesizes text to local WAV artifacts
- a subprocess playback adapter runs those artifacts through `afplay`
- Vosk recognizes a constrained Italian command grammar from the microphone

The core still sees only ports plus structured results. It does not know about
Piper flags, Vosk model loading, or subprocess signaling details.

## Rules

- the core depends only on ports
- real providers live in adapters or infra, never in the core
- CLI code composes concrete implementations but does not become business logic
- future providers may be local, hybrid, or remote, but the domain model stays
  unchanged
- provider configuration must remain explicit and local-first

## Deferred Work

Provider quality, latency tuning, device access, sentence-accurate progress,
model lifecycle management, and richer streaming loops are still future
concerns. This repository establishes the shape required to support them safely
later.
