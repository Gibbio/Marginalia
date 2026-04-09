KOKORO_ASSETS_DIR  ?= .kokoro-assets
VOSK_MODEL_URL     ?= https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip
VOSK_MODEL_NAME    ?= vosk-model-small-it-0.22
MODELS_DIR         ?= .models
VOSK_LIB_VERSION   ?= 0.3.45
VOSK_LIB_DIR       ?= .vosk-lib
ORT_VERSION        ?= 1.20.1
WHISPER_MODEL_DIR  ?= .models/whisper
WHISPER_MODEL_NAME ?= ggml-base.bin
WHISPER_MODEL_URL  ?= https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$(WHISPER_MODEL_NAME)

.PHONY: \
	bootstrap-beta bootstrap-kokoro bootstrap-ort bootstrap-vosk bootstrap-vosk-lib \
	_kokoro-hf-cli _kokoro-curl \
	tui-rs beta-test beta-doctor \
	setup bootstrap bootstrap-runtime-deps bootstrap-providers \
	bootstrap-kokoro-python bootstrap-whisper bootstrap-system-deps setup-config \
	format lint test smoke run-cli-help doctor \
	clean clean-alpha clean-session

# ---------------------------------------------------------------------------
# Beta — provider bootstrapping
# ---------------------------------------------------------------------------

# Download all Beta providers in one shot.
bootstrap-beta: bootstrap-kokoro bootstrap-vosk bootstrap-vosk-lib bootstrap-whisper
	@echo ""
	@echo "Beta providers ready. Run 'make beta-doctor' to verify."

KOKORO_HF_REPO ?= hexgrad/Kokoro-82M
# Voices to download when using curl fallback (space-separated, without extension).
KOKORO_VOICES  ?= af af_bella af_sarah am_adam am_michael bf_emma bm_george

# Download Kokoro ONNX model assets and ONNX Runtime library.
# Uses huggingface-cli if available, otherwise falls back to plain curl.
bootstrap-kokoro: bootstrap-ort
	@echo "Downloading Kokoro ONNX model assets ($(KOKORO_HF_REPO))..."
	@mkdir -p $(KOKORO_ASSETS_DIR)/voices
	@if command -v huggingface-cli >/dev/null 2>&1; then \
		$(MAKE) _kokoro-hf-cli; \
	else \
		echo "huggingface-cli not found, falling back to curl..."; \
		$(MAKE) _kokoro-curl; \
	fi
	@echo ""
	@echo "Kokoro assets ready at $(KOKORO_ASSETS_DIR)/. Run 'make beta-doctor' to verify."

_kokoro-hf-cli:
	huggingface-cli download $(KOKORO_HF_REPO) kokoro.onnx config.json \
		--local-dir $(KOKORO_ASSETS_DIR)
	huggingface-cli download $(KOKORO_HF_REPO) \
		--include "voices/*" --local-dir $(KOKORO_ASSETS_DIR)

_kokoro-curl:
	@HF="https://huggingface.co/$(KOKORO_HF_REPO)/resolve/main"; \
	for FILE in kokoro.onnx config.json; do \
		DEST="$(KOKORO_ASSETS_DIR)/$$FILE"; \
		if [ -f "$$DEST" ]; then \
			echo "  $$FILE already present, skipping."; \
		else \
			echo "  Downloading $$FILE..."; \
			curl -fL --progress-bar -o "$$DEST" "$$HF/$$FILE" || { echo "Failed: $$FILE"; exit 1; }; \
		fi; \
	done; \
	for VOICE in $(KOKORO_VOICES); do \
		DEST="$(KOKORO_ASSETS_DIR)/voices/$$VOICE.bin"; \
		if [ -f "$$DEST" ]; then \
			echo "  voices/$$VOICE.bin already present, skipping."; \
		else \
			echo "  Downloading voices/$$VOICE.bin..."; \
			curl -fL --progress-bar -o "$$DEST" "$$HF/voices/$$VOICE.bin" 2>/dev/null \
				|| { echo "  (voices/$$VOICE.bin not available on HF, skipping)"; rm -f "$$DEST"; }; \
		fi; \
	done

# Download ONNX Runtime dynamic library for the current platform.
bootstrap-ort:
	@echo "Downloading ONNX Runtime v$(ORT_VERSION)..."
	@mkdir -p $(KOKORO_ASSETS_DIR)/lib
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	if [ "$$OS" = "Darwin" ]; then \
		if [ "$$ARCH" = "arm64" ]; then \
			URL="https://github.com/microsoft/onnxruntime/releases/download/v$(ORT_VERSION)/onnxruntime-osx-arm64-$(ORT_VERSION).tgz"; \
		else \
			URL="https://github.com/microsoft/onnxruntime/releases/download/v$(ORT_VERSION)/onnxruntime-osx-x86_64-$(ORT_VERSION).tgz"; \
		fi; \
		LIB_GLOB="libonnxruntime*.dylib"; \
	elif [ "$$OS" = "Linux" ]; then \
		if [ "$$ARCH" = "aarch64" ] || [ "$$ARCH" = "arm64" ]; then \
			URL="https://github.com/microsoft/onnxruntime/releases/download/v$(ORT_VERSION)/onnxruntime-linux-aarch64-$(ORT_VERSION).tgz"; \
		else \
			URL="https://github.com/microsoft/onnxruntime/releases/download/v$(ORT_VERSION)/onnxruntime-linux-x64-$(ORT_VERSION).tgz"; \
		fi; \
		LIB_GLOB="libonnxruntime.so*"; \
	else \
		echo "Unsupported OS: $$OS"; exit 1; \
	fi; \
	if ls $(KOKORO_ASSETS_DIR)/lib/libonnxruntime* >/dev/null 2>&1; then \
		echo "ONNX Runtime already present in $(KOKORO_ASSETS_DIR)/lib/, skipping."; \
	else \
		echo "Downloading $$URL..."; \
		curl -L -o /tmp/ort.tgz "$$URL" && \
		tar -xzf /tmp/ort.tgz -C /tmp && \
		find /tmp/onnxruntime-* -name "$$LIB_GLOB" -exec cp {} $(KOKORO_ASSETS_DIR)/lib/ \; && \
		rm -rf /tmp/ort.tgz /tmp/onnxruntime-* && \
		echo "ONNX Runtime installed at $(KOKORO_ASSETS_DIR)/lib/"; \
	fi

# Download Vosk acoustic model (Italian, small).
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

# Download Vosk native library (libvosk.so / libvosk.dylib).
# On Linux also ensures libasound2-dev is installed (required by cpal).
bootstrap-vosk-lib:
	@OS=$$(uname -s); \
	if [ "$$OS" = "Linux" ]; then \
		if ! pkg-config --exists alsa 2>/dev/null; then \
			echo "Installing libasound2-dev (required by cpal on Linux)..."; \
			sudo apt-get install -y libasound2-dev; \
		fi; \
	fi
	@echo "Downloading Vosk native library v$(VOSK_LIB_VERSION)..."
	@mkdir -p $(VOSK_LIB_DIR)
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	if [ "$$OS" = "Darwin" ]; then \
		URL="https://github.com/alphacep/vosk-api/releases/download/v$(VOSK_LIB_VERSION)/vosk-osx-universal-$(VOSK_LIB_VERSION).zip"; \
		LIB="libvosk.dylib"; \
	elif [ "$$OS" = "Linux" ]; then \
		if [ "$$ARCH" = "aarch64" ] || [ "$$ARCH" = "arm64" ]; then \
			URL="https://github.com/alphacep/vosk-api/releases/download/v$(VOSK_LIB_VERSION)/vosk-linux-aarch64-$(VOSK_LIB_VERSION).zip"; \
		else \
			URL="https://github.com/alphacep/vosk-api/releases/download/v$(VOSK_LIB_VERSION)/vosk-linux-x86_64-$(VOSK_LIB_VERSION).zip"; \
		fi; \
		LIB="libvosk.so"; \
	else \
		echo "Unsupported OS: $$OS"; exit 1; \
	fi; \
	if [ -f "$(VOSK_LIB_DIR)/$$LIB" ]; then \
		echo "$$LIB already present in $(VOSK_LIB_DIR)/, skipping."; \
	else \
		echo "Downloading $$URL..."; \
		curl -L -o /tmp/vosk-lib.zip "$$URL" && \
		unzip -qo /tmp/vosk-lib.zip -d /tmp/vosk-lib-extract && \
		find /tmp/vosk-lib-extract -name "libvosk.*" -exec cp {} $(VOSK_LIB_DIR)/ \; && \
		rm -rf /tmp/vosk-lib.zip /tmp/vosk-lib-extract && \
		echo "Vosk library installed at $(VOSK_LIB_DIR)/$$LIB"; \
	fi

# Download Whisper ggml model for dictation transcription.
# Uses ggml-base (multilingual, ~145 MB) from the whisper.cpp HuggingFace repo.
# Override WHISPER_MODEL_NAME to use a different model (e.g. ggml-small.bin).
# NOTE: building marginalia-stt-whisper also requires cmake and libclang-dev.
#       On Debian/Ubuntu: sudo apt-get install -y cmake libclang-dev
bootstrap-whisper:
	@echo "Downloading Whisper model ($(WHISPER_MODEL_NAME))..."
	@mkdir -p $(WHISPER_MODEL_DIR)
	@if [ -f "$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)" ]; then \
		echo "Whisper model already present, skipping."; \
	else \
		echo "  Downloading $(WHISPER_MODEL_URL)..."; \
		curl -fL --progress-bar -o "$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)" "$(WHISPER_MODEL_URL)" \
			|| { echo "Failed to download Whisper model"; exit 1; }; \
		echo "Whisper model installed at $(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)"; \
	fi

# ---------------------------------------------------------------------------
# Beta — run and verify
# ---------------------------------------------------------------------------

# Launch the Beta TUI. Detects available providers automatically:
#   - stt=vosk     if $(VOSK_LIB_DIR)/libvosk.* and $(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME) exist
#   - tts=kokoro   if $(KOKORO_ASSETS_DIR)/ exists
#   - dictation=whisper if $(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME) exists
# Run 'make bootstrap-beta' first to install providers.
tui-rs:
	@VOSK_LIB=$$(ls $(VOSK_LIB_DIR)/libvosk.* 2>/dev/null | head -1); \
	VOSK_MODEL=$(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME); \
	KOKORO_DIR=$(KOKORO_ASSETS_DIR); \
	WHISPER_MODEL=$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME); \
	echo ""; \
	echo "=== marginalia-tui — provider check ==="; \
	if [ -n "$$VOSK_LIB" ]; then \
		echo "  stt:       vosk  ($$VOSK_LIB)"; \
		if [ -d "$$VOSK_MODEL" ]; then \
			echo "  stt model: $$VOSK_MODEL"; \
		else \
			echo "  stt model: MISSING ($$VOSK_MODEL) — stt → fake"; \
			VOSK_LIB=""; \
		fi; \
	else \
		echo "  stt:       fake  (run 'make bootstrap-vosk-lib bootstrap-vosk' to enable)"; \
	fi; \
	if [ -d "$$KOKORO_DIR" ]; then \
		echo "  tts:       kokoro  ($$KOKORO_DIR)"; \
	else \
		echo "  tts:       fake  (run 'make bootstrap-kokoro' to enable)"; \
	fi; \
	if [ -f "$$WHISPER_MODEL" ]; then \
		echo "  dictation: whisper  ($$WHISPER_MODEL)"; \
	else \
		echo "  dictation: fake  (run 'make bootstrap-whisper' to enable)"; \
		WHISPER_MODEL=""; \
	fi; \
	echo "======================================="; \
	echo ""; \
	FEATURES=""; \
	if [ -n "$$VOSK_LIB" ]; then FEATURES="vosk-stt"; fi; \
	if [ -n "$$WHISPER_MODEL" ]; then \
		if [ -n "$$FEATURES" ]; then FEATURES="$$FEATURES,whisper-stt"; \
		else FEATURES="whisper-stt"; fi; \
	fi; \
	VOSK_PATH=$(VOSK_LIB_DIR) \
	MARGINALIA_VOSK_MODEL=$$VOSK_MODEL \
	LD_LIBRARY_PATH=$(VOSK_LIB_DIR):$$LD_LIBRARY_PATH \
	DYLD_LIBRARY_PATH=$(VOSK_LIB_DIR):$$DYLD_LIBRARY_PATH \
	MARGINALIA_KOKORO_ASSETS=$$KOKORO_DIR \
	MARGINALIA_WHISPER_MODEL=$$WHISPER_MODEL \
	cargo run --manifest-path apps/tui-rs/Cargo.toml \
		$$([ -n "$$FEATURES" ] && echo "--features $$FEATURES")

beta-test:
	cargo test

beta-doctor:
	cargo run -p marginalia-devtools -- kokoro-doctor --assets-root $(KOKORO_ASSETS_DIR)

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------

clean:
	rm -rf target/
	@echo "Rust build artifacts cleaned."

clean-alpha:
	rm -rf build/ dist/ *.egg-info .eggs/
	rm -rf .mypy_cache/ .pytest_cache/ .ruff_cache/ .coverage htmlcov/
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	@echo "Alpha Python build artifacts cleaned."

clean-session:
	rm -rf .marginalia/
	@echo "Session data cleaned."

# ---------------------------------------------------------------------------
# Alpha reference (migration reference only — do not use for Beta development)
# ---------------------------------------------------------------------------

PYTHON           ?= python3
VENV_DIR         ?= .venv
VENV_PYTHON      := $(VENV_DIR)/bin/python
VENV_PIP         := $(VENV_PYTHON) -m pip
PYTHONPATH_LOCAL := apps/backend/src:apps/cli/src:packages/core/src:packages/adapters/src:packages/infra/src

setup: bootstrap-system-deps bootstrap bootstrap-runtime-deps bootstrap-providers setup-config
	@echo "Alpha setup complete. Run 'make doctor' to verify."

bootstrap-system-deps:
	@echo "Checking system dependencies (macOS/Homebrew)..."
	@command -v brew >/dev/null 2>&1 || { echo "Error: Homebrew required."; exit 1; }
	@brew list portaudio >/dev/null 2>&1 || brew install portaudio
	@brew list espeak-ng >/dev/null 2>&1 || brew install espeak-ng
	@command -v uv >/dev/null 2>&1 || brew install uv
	@echo "System dependencies OK."

bootstrap:
	$(PYTHON) -m venv $(VENV_DIR)
	$(VENV_PIP) install --upgrade pip
	$(VENV_PIP) install -e ".[dev]"

bootstrap-runtime-deps:
	$(VENV_PIP) install vosk sounddevice numpy

bootstrap-providers: bootstrap-kokoro-python bootstrap-vosk bootstrap-whisper
	@echo "Alpha providers bootstrapped."

# Alpha Python Kokoro (not used by Beta runtime).
bootstrap-kokoro-python:
	@echo "Setting up Alpha Python Kokoro TTS..."
	uv venv .venv-kokoro --python 3.12 --seed --clear
	uv pip install --python .venv-kokoro/bin/python "kokoro>=0.9.4,<1.0" soundfile

bootstrap-whisper:
	@echo "Cloning and building whisper.cpp..."
	git clone --depth 1 https://github.com/ggerganov/whisper.cpp .whisper-cpp || true
	cd .whisper-cpp && make -j
	cd .whisper-cpp && ./models/download-ggml-model.sh base
	@echo "whisper.cpp ready."

setup-config:
	@if [ -f marginalia.toml ]; then \
		echo "Config file marginalia.toml already exists, skipping."; \
	else \
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
	$(VENV_DIR)/bin/mypy apps/backend/src apps/cli/src packages/core/src packages/adapters/src packages/infra/src tests

test:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_DIR)/bin/pytest

smoke:
	./scripts/smoke.sh

doctor:
	PYTHONPATH=$(PYTHONPATH_LOCAL) MARGINALIA_CONFIG=marginalia.toml $(VENV_PYTHON) -m marginalia_cli doctor

run-cli-help:
	PYTHONPATH=$(PYTHONPATH_LOCAL) $(VENV_PYTHON) -m marginalia_cli --help
