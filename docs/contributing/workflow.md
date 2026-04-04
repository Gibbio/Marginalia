# Development Workflow

## Principles

- keep changes coherent and reviewable
- update docs and ADRs alongside architecture changes
- keep the core independent from UI and editor concerns
- prefer explicit placeholders over fake completeness

## Typical Change Flow

1. identify the relevant package boundary first
2. update or add tests for implemented behavior
3. make the code change
4. update docs if scope, structure, or workflow changed
5. update an ADR if the architectural decision changed
6. run `make lint`, `make test`, and `make smoke`

## Commit Style

Use small, intention-revealing commits such as:

- `chore: bootstrap repository structure`
- `docs: add product vision and architecture docs`
- `feat: add cli and storage skeleton`
- `ci: add validation workflow`

## ADR Expectations

Architecture is not allowed to drift silently. If a change affects:

- repository shape
- interface strategy
- provider architecture
- persistence strategy
- editor coupling

then the relevant ADR must be updated or a new one must be added.

## Provider Work

Real provider integrations are welcome later, but they must:

- live behind ports
- avoid leaking provider SDK details into the core
- keep network assumptions explicit
- preserve local-first operation where possible

## Editor Integrations

Editor adapters are future work. Do not add direct editor dependencies to the
domain or application layers. Any future adapter should consume exported
contracts rather than reach into the core internals.
