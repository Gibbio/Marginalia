# Marginalia Product One-Pager

## Summary

Marginalia is a local AI-first voice reading and annotation engine for people who
work deeply with long-form text. It should read documents aloud, react to voice
commands, let the user dictate notes anchored to the current reading location,
and later use those notes to rewrite or summarize relevant parts of a document.

## Who It Is For

Primary users:

- writers reviewing drafts by ear
- researchers reading dense material while walking or commuting
- knowledge workers who think better by dictating reactions while listening

These users do not need a collaborative cloud platform first. They need a
reliable local tool that helps them keep reading context and note context aligned.

## Problem

Today, listening, annotating, and revising are fragmented:

- text-to-speech tools read well but do not preserve note anchors
- note apps capture thoughts but lose the exact reading position
- rewrite workflows usually happen later and without the original listening context

The result is friction between first-pass comprehension and later revision.

## Product Promise

Marginalia should make it possible to:

1. ingest a text document locally
2. read it aloud as if it were an audiobook
3. pause or resume with voice-oriented controls
4. capture anchored dictated notes without losing place
5. later ask for rewrites or summaries grounded in the note trail

## Why Local First

- reading and note capture should work with low latency
- personal drafts and annotations are often sensitive
- the product should still be useful before any networked features exist
- local-first design discourages premature distributed architecture

## Scope Now

Current scope is deliberately foundational:

- well-architected Python core
- CLI as the first usable interface
- SQLite persistence
- swappable STT, TTS, playback, and LLM ports
- fake provider implementations for bootstrapping
- engineering hygiene, docs, ADRs, and CI

## Scope Later

Later but intentionally deferred:

- desktop shell
- local HTTP or WebSocket API
- editor adapters such as Obsidian
- production STT/TTS integrations
- richer search over notes and documents

## Success Criteria For The Current Phase

- the repo supports disciplined engineering for months, not days
- CLI flows demonstrate ingestion, session state, note anchoring, and search
- architecture makes provider replacement and future UI surfaces straightforward
- product decisions are captured explicitly in ADRs and docs
