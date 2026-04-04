# ADR 0004: Swappable STT and TTS Providers

- Status: Accepted

## Context

Speech tooling changes quickly. Users may prefer different local or hybrid
providers based on quality, latency, privacy, device access, or cost. Locking
the product to one provider too early would make future adaptation expensive.

## Decision

Model STT, TTS, playback, and LLM interactions behind explicit ports and keep
concrete providers in adapter or infrastructure layers.

## Consequences

- fake providers can support early development safely
- real providers can be added later with less churn
- interface design becomes an explicit architectural concern

## Alternatives Considered

- direct SDK usage in CLI code: rejected because it would create hard coupling
- waiting to abstract until later: rejected because the speech boundary is a
  core product constraint, not an incidental implementation detail
