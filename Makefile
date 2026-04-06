PYTHON ?= python3
VENV_DIR ?= .venv
VENV_PYTHON := $(VENV_DIR)/bin/python
VENV_PIP := $(VENV_PYTHON) -m pip
PYTHONPATH_LOCAL := apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src

.PHONY: bootstrap bootstrap-kokoro bootstrap-whisper format lint test smoke run-cli-help

bootstrap:
	$(PYTHON) -m venv $(VENV_DIR)
	$(VENV_PIP) install --upgrade pip
	$(VENV_PIP) install -e ".[dev]"

bootstrap-kokoro:
	uv venv .venv-kokoro --python 3.12 --seed --clear
	uv pip install --python .venv-kokoro/bin/python "kokoro>=0.9.4,<1.0" soundfile

bootstrap-whisper:
	@echo "Cloning and building whisper.cpp..."
	git clone --depth 1 https://github.com/ggerganov/whisper.cpp .whisper-cpp || true
	cd .whisper-cpp && make -j
	@echo "Downloading ggml-base model..."
	cd .whisper-cpp && ./models/download-ggml-model.sh base
	@echo ""
	@echo "Done. Add to your config:"
	@echo '  [whisper_cpp]'
	@echo '  executable = ".whisper-cpp/main"'
	@echo '  model_path = ".whisper-cpp/models/ggml-base.bin"'

format:
	$(VENV_DIR)/bin/ruff format .
	$(VENV_DIR)/bin/ruff check . --fix

lint:
	$(VENV_DIR)/bin/ruff check .
	$(VENV_DIR)/bin/mypy apps/cli/src packages/core/src packages/adapters/src packages/infra/src tests

test:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_DIR)/bin/pytest

smoke:
	./scripts/smoke.sh

run-cli-help:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli --help
