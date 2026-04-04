PYTHON ?= python3
VENV_DIR ?= .venv
VENV_PYTHON := $(VENV_DIR)/bin/python
VENV_PIP := $(VENV_PYTHON) -m pip
PYTHONPATH_LOCAL := apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src

.PHONY: bootstrap format lint test smoke run-cli-help

bootstrap:
	$(PYTHON) -m venv $(VENV_DIR)
	$(VENV_PIP) install --upgrade pip
	$(VENV_PIP) install -e ".[dev]"

format:
	$(VENV_DIR)/bin/ruff format .
	$(VENV_DIR)/bin/ruff check . --fix

lint:
	$(VENV_DIR)/bin/ruff check .
	$(VENV_DIR)/bin/mypy apps/cli/src packages/core/src packages/adapters/src packages/infra/src tests

test:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_DIR)/bin/pytest

smoke:
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_DB_PATH=.marginalia/smoke.sqlite3 $(VENV_PYTHON) -m marginalia_cli doctor --json
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_DB_PATH=.marginalia/smoke.sqlite3 $(VENV_PYTHON) -m marginalia_cli ingest examples/sample-document.txt --json
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_DB_PATH=.marginalia/smoke.sqlite3 $(VENV_PYTHON) -m marginalia_cli status --json

run-cli-help:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli --help
