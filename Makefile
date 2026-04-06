PYTHON ?= python3
VENV_DIR ?= .venv
VENV_PYTHON := $(VENV_DIR)/bin/python
VENV_PIP := $(VENV_PYTHON) -m pip
PYTHONPATH_LOCAL := apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src
VOSK_MODEL_URL ?= https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip
VOSK_MODEL_NAME ?= vosk-model-small-it-0.22
MODELS_DIR ?= .models

.PHONY: bootstrap bootstrap-kokoro bootstrap-vosk bootstrap-whisper bootstrap-providers \
        bootstrap-system-deps setup format lint test smoke run-cli-help doctor

# ---------------------------------------------------------------------------
# Full setup — one command to get everything running
# ---------------------------------------------------------------------------

setup: bootstrap-system-deps bootstrap bootstrap-runtime-deps bootstrap-providers setup-config
	@echo ""
	@echo "============================================================"
	@echo "  Setup complete. Running doctor to verify..."
	@echo "============================================================"
	@echo ""
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli doctor
	@echo ""
	@echo "============================================================"
	@echo "  Ready! Start with:"
	@echo "    PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli shell"
	@echo "  or:"
	@echo "    make shell"
	@echo "============================================================"

# ---------------------------------------------------------------------------
# System dependencies (macOS / Homebrew)
# ---------------------------------------------------------------------------

bootstrap-system-deps:
	@echo "Checking system dependencies..."
	@command -v brew >/dev/null 2>&1 || { echo "Error: Homebrew is required. Install from https://brew.sh"; exit 1; }
	@brew list portaudio >/dev/null 2>&1 || { echo "Installing portaudio..."; brew install portaudio; }
	@brew list espeak-ng >/dev/null 2>&1 || { echo "Installing espeak-ng..."; brew install espeak-ng; }
	@command -v uv >/dev/null 2>&1 || { echo "Installing uv..."; brew install uv; }
	@echo "System dependencies OK."

# ---------------------------------------------------------------------------
# Python environment
# ---------------------------------------------------------------------------

bootstrap:
	$(PYTHON) -m venv $(VENV_DIR)
	$(VENV_PIP) install --upgrade pip
	$(VENV_PIP) install -e ".[dev]"

bootstrap-runtime-deps:
	@echo "Installing runtime Python packages..."
	$(VENV_PIP) install vosk sounddevice numpy

# ---------------------------------------------------------------------------
# Provider setup
# ---------------------------------------------------------------------------

bootstrap-providers: bootstrap-kokoro bootstrap-vosk bootstrap-whisper
	@echo "All providers bootstrapped."

bootstrap-kokoro:
	@echo "Setting up Kokoro TTS..."
	uv venv .venv-kokoro --python 3.12 --seed --clear
	uv pip install --python .venv-kokoro/bin/python "kokoro>=0.9.4,<1.0" soundfile

bootstrap-vosk:
	@echo "Downloading Vosk model ($(VOSK_MODEL_NAME))..."
	@mkdir -p $(MODELS_DIR)/vosk
	@if [ -d "$(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME)" ]; then \
		echo "Vosk model already present, skipping."; \
	else \
		curl -L -o /tmp/$(VOSK_MODEL_NAME).zip $(VOSK_MODEL_URL) && \
		unzip -qo /tmp/$(VOSK_MODEL_NAME).zip -d $(MODELS_DIR)/vosk && \
		rm /tmp/$(VOSK_MODEL_NAME).zip && \
		echo "Vosk model installed at $(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME)"; \
	fi

bootstrap-whisper:
	@echo "Cloning and building whisper.cpp..."
	git clone --depth 1 https://github.com/ggerganov/whisper.cpp .whisper-cpp || true
	cd .whisper-cpp && make -j
	@echo "Downloading ggml-base model..."
	cd .whisper-cpp && ./models/download-ggml-model.sh base
	@echo "whisper.cpp ready."

# ---------------------------------------------------------------------------
# Config generation
# ---------------------------------------------------------------------------

setup-config:
	@if [ -f marginalia.toml ]; then \
		echo "Config file marginalia.toml already exists, skipping."; \
	else \
		echo "Generating marginalia.toml..."; \
		ROOT_DIR=$$(pwd); \
		cat > marginalia.toml <<-TOML_EOF
	environment = "local"
	log_level = "INFO"
	database_path = ".marginalia/marginalia.sqlite3"
	audio_cache_dir = ".marginalia/audio-cache"
	command_language = "it"

	[providers]
	command_stt = "vosk"
	dictation_stt = "whisper-cpp"
	tts = "kokoro"
	playback = "subprocess"
	llm = "fake"
	allow_fallback = true

	[kokoro]
	python_executable = "$$ROOT_DIR/.venv-kokoro/bin/python"
	default_voice = "if_sara"
	lang_code = "i"
	speed = 1.0

	[vosk]
	model_path = "$$ROOT_DIR/.models/vosk/$(VOSK_MODEL_NAME)"
	sample_rate = 16000
	timeout_seconds = 4.0

	[whisper_cpp]
	executable = "$$ROOT_DIR/.whisper-cpp/main"
	model_path = "$$ROOT_DIR/.whisper-cpp/models/ggml-base.bin"
	language = "it"
	max_record_seconds = 120

	[playback]
	command = "afplay"
	TOML_EOF
		echo "Config generated at marginalia.toml"; \
	fi

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

doctor:
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_CONFIG=marginalia.toml $(VENV_PYTHON) -m marginalia_cli doctor

shell:
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_CONFIG=marginalia.toml $(VENV_PYTHON) -m marginalia_cli shell

run-cli-help:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli --help
