# ADR 0010: Adopt Kokoro and Target ONNX Runtime

- Status: Accepted

## Context

Marginalia needs a local-first TTS stack that can survive beyond the current
Alpha CLI and desktop development phase.

The likely product path now includes:

- desktop applications
- Android clients
- iOS clients

That changes the TTS decision materially. The project does not only need a
high-quality local voice today; it needs a model and runtime strategy that can
be carried across desktop and mobile without splitting the product into
unrelated provider families.

The current Kokoro integration uses a dedicated Python runtime and worker
process. That is acceptable for the current desktop Alpha, but it is not a
credible long-term runtime story for Android or iOS Beta hosts.

The project also does not want to make cloud TTS the architectural default.
Marginalia is local-first, and its core reading loop should not depend on
network availability or third-party service latency.

## Decision

Adopt Kokoro as the canonical local TTS model family for Marginalia Beta.

Adopt ONNX Runtime as the preferred execution runtime for Kokoro across desktop
and mobile, where platform support and validation allow it.

This means:

- Kokoro is the default long-term TTS direction for the product
- ONNX Runtime is the preferred long-term inference layer for that direction
- the current Python Kokoro worker remains a transitional desktop adapter, not
  the final cross-platform runtime design
- TTS remains behind the existing speech synthesis port, so runtime-specific
  adapters can vary without leaking into the core

Platform intent:

- desktop: prefer Kokoro via ONNX Runtime when ready; keep the Python worker as
  the near-term implementation path during Beta transition
- Android: target ONNX Runtime Mobile with device-appropriate acceleration
- iOS: target ONNX Runtime Mobile with device-appropriate acceleration

Marginalia should optimize for one portable local TTS model family before
adding optional higher-complexity alternatives.

## Rationale

Kokoro is the best current fit for Marginalia's product constraints:

- strong quality relative to its size
- lower operational weight than larger high-end local TTS stacks
- credible path to mobile deployment
- good fit for chunk-based reading workloads
- local execution compatible with the project's architectural constraints

ONNX Runtime is the best current fit for the runtime layer because it provides
a realistic path to keeping one inference strategy across desktop and mobile
instead of inventing separate platform-specific TTS systems.

## Consequences

- Marginalia now has a canonical local TTS direction instead of an open-ended
  provider search
- desktop and mobile can converge on one model family over time
- the current Python Kokoro adapter should be treated as an implementation
  bridge, not a destination architecture
- future TTS work should avoid depending on Python-specific Kokoro behavior if
  that behavior is not portable to ONNX Runtime
- model packaging, caching, and versioning become more important product
  concerns
- runtime validation must include platform-specific acceleration and memory
  behavior on desktop, Android, and iOS

## Non-Goals

This decision does not require:

- immediate replacement of the current desktop Kokoro worker
- identical binary packaging on every platform
- removal of the provider abstraction
- prohibition of future optional TTS providers

It does require that optional future providers remain secondary to the main
cross-platform local TTS path unless product constraints clearly change.

## Alternatives Considered

- XTTS as the canonical local TTS direction: rejected because it is too heavy
  for the intended desktop-plus-mobile product path
- cloud TTS as the primary product default: rejected because it conflicts with
  local-first operation and introduces network dependency into the core reading
  loop
- platform-native OS TTS as the primary voice engine: rejected because it would
  fragment voice quality, behavior, and product consistency across platforms
- keeping Kokoro only through a Python runtime: rejected because it does not
  provide a credible path to Android and iOS
