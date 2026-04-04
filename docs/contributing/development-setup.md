# Development Setup

## Baseline

Marginalia is developed as a Python monorepo with a CLI-first workflow.

Recommended prerequisites:

- Python 3.12+
- `make`
- optional: VS Code with the recommended Python and Ruff extensions

## Bootstrap

```bash
make bootstrap
```

This creates `.venv`, upgrades `pip`, and installs the project plus development
dependencies in editable mode.

## Daily Commands

```bash
make format
make lint
make test
make smoke
make run-cli-help
```

## Configuration

Useful environment variables:

- `MARGINALIA_HOME`
- `MARGINALIA_DATA_DIR`
- `MARGINALIA_DB_PATH`
- `MARGINALIA_LOG_LEVEL`
- `MARGINALIA_CONFIG`

The CLI `doctor` command reports the effective local configuration.

## Devcontainer

A lightweight devcontainer is included so work can resume quickly on another
machine. It intentionally mirrors the local setup instead of introducing a
separate orchestration layer.

## Home And Office Development

The product is expected to evolve across mixed contexts:

- home: better for architecture, docs, ADRs, deeper model thinking, provider research
- office: better for bounded implementation, test hardening, CI fixes, and routine polishing

This distinction is reflected in the backlog seed with explicit context tags.
