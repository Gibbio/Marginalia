KOKORO_ASSETS_DIR  ?= models/tts/kokoro
VOSK_MODEL_URL     ?= https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip
VOSK_MODEL_NAME    ?= vosk-model-small-it-0.22
MODELS_DIR         ?= models/stt
VOSK_LIB_VERSION   ?= 0.3.42
VOSK_LIB_DIR       ?= models/stt/vosk
ORT_VERSION        ?= 1.24.4
WHISPER_MODEL_DIR  ?= models/stt/whisper
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

KOKORO_HF_REPO ?= onnx-community/Kokoro-82M-ONNX
# Voices to download when using curl fallback (space-separated, without extension).
KOKORO_VOICES  ?= af af_bella af_sarah am_adam am_michael bf_emma bm_george

# Download Kokoro ONNX model assets and ONNX Runtime library.
# Uses huggingface-cli if available, otherwise falls back to plain curl.
bootstrap-kokoro: bootstrap-ort
	@echo "Downloading Kokoro ONNX model assets ($(KOKORO_HF_REPO))..."
	@mkdir -p $(KOKORO_ASSETS_DIR)/voices
	@if command -v hf >/dev/null 2>&1; then \
		$(MAKE) _kokoro-hf-cli HF_CLI=hf; \
	elif command -v huggingface-cli >/dev/null 2>&1; then \
		$(MAKE) _kokoro-hf-cli HF_CLI=huggingface-cli; \
	else \
		echo "hf/huggingface-cli not found, falling back to curl..."; \
		$(MAKE) _kokoro-curl; \
	fi
	@echo ""
	@echo "Kokoro assets ready at $(KOKORO_ASSETS_DIR)/. Run 'make beta-doctor' to verify."

HF_CLI ?= hf
_kokoro-hf-cli:
	$(HF_CLI) download $(KOKORO_HF_REPO) onnx/model_q8f16.onnx \
		--local-dir $(KOKORO_ASSETS_DIR)
	$(HF_CLI) download hexgrad/Kokoro-82M config.json \
		--local-dir $(KOKORO_ASSETS_DIR)
	$(HF_CLI) download $(KOKORO_HF_REPO) \
		--include "voices/*" --local-dir $(KOKORO_ASSETS_DIR)

_kokoro-curl:
	@HF="https://huggingface.co/$(KOKORO_HF_REPO)/resolve/main"; \
	DEST="$(KOKORO_ASSETS_DIR)/model.onnx"; \
	if [ -f "$$DEST" ]; then \
		echo "  model.onnx already present, skipping."; \
	else \
		echo "  Downloading onnx/model_q8f16.onnx..."; \
		curl -fL --progress-bar -o "$$DEST" "$$HF/onnx/model_q8f16.onnx" || { echo "Failed: model.onnx"; exit 1; }; \
	fi; \
	DEST="$(KOKORO_ASSETS_DIR)/config.json"; \
	if [ -f "$$DEST" ]; then \
		echo "  config.json already present, skipping."; \
	else \
		echo "  Downloading config.json (from hexgrad/Kokoro-82M)..."; \
		curl -fL --progress-bar -o "$$DEST" "https://huggingface.co/hexgrad/Kokoro-82M/resolve/main/config.json" \
			|| { echo "Failed: config.json"; exit 1; }; \
	fi; \
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
		URL="https://github.com/alphacep/vosk-api/releases/download/v$(VOSK_LIB_VERSION)/vosk-osx-$(VOSK_LIB_VERSION).zip"; \
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
# NOTE: building marginalia-stt-whisper compiles whisper.cpp from source and requires:
#   macOS:          brew install cmake   (libclang comes with Xcode CLT)
#   Debian/Ubuntu:  sudo apt-get install -y cmake libclang-dev
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
TUI_TOML     := apps/tui-rs/marginalia.toml
TUI_TEMPLATE := apps/tui-rs/marginalia.toml.template

# Generate marginalia.toml from template based on platform and available providers.
$(TUI_TOML): $(TUI_TEMPLATE)
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	PLATFORM="$$OS $$ARCH"; \
	DATE=$$(date "+%Y-%m-%d %H:%M"); \
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then \
		TTS_SECTION='# Kokoro via MLX Metal GPU (auto-download da HuggingFace)\n[mlx]\nvoice = "if_sara"    # Italian female (or: im_nicola, af_bella, am_adam)'; \
	elif [ -d "$(KOKORO_ASSETS_DIR)" ]; then \
		TTS_SECTION='# Kokoro via ONNX Runtime CPU\n[kokoro]\nassets_root = "$(KOKORO_ASSETS_DIR)"\nphonemizer_program = "espeak-ng"\nphonemizer_args = ["-v", "it", "--ipa", "-q"]'; \
	else \
		TTS_SECTION='# Nessun TTS configurato. Esegui: make bootstrap-kokoro'; \
	fi; \
	if [ -d "$(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME)" ]; then \
		VOSK_SECTION='[vosk]\nmodel_path = "$(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME)"\ncommands = ["pausa", "avanti", "indietro", "stop"]\n# speech_threshold = 3000  # 0-32767, higher = less sensitive (default: 3000)\n# silence_timeout = 1.2    # seconds of silence before finalizing (default: 1.2)\n# min_speech_ms = 300      # minimum speech duration to accept (default: 300)'; \
	else \
		VOSK_SECTION='# [vosk]  — non installato. Esegui: make bootstrap-vosk bootstrap-vosk-lib'; \
	fi; \
	if [ -f "$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)" ]; then \
		WHISPER_SECTION='[whisper]\nmodel_path = "$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)"\nlanguage = "it"'; \
	else \
		WHISPER_SECTION='# [whisper]  — non installato. Esegui: make bootstrap-whisper'; \
	fi; \
	sed -e "s|__PLATFORM__|$$PLATFORM|" \
	    -e "s|__DATE__|$$DATE|" \
	    -e "s|__TTS_SECTION__|$$TTS_SECTION|" \
	    -e "s|__VOSK_SECTION__|$$VOSK_SECTION|" \
	    -e "s|__WHISPER_SECTION__|$$WHISPER_SECTION|" \
	    $(TUI_TEMPLATE) > $(TUI_TOML); \
	echo "Generated $(TUI_TOML) for $$PLATFORM"

tui-rs: $(TUI_TOML)
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	VOSK_LIB=$$(ls $(VOSK_LIB_DIR)/libvosk.* 2>/dev/null | head -1); \
	WHISPER_MODEL=$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME); \
	FEATURES=""; \
	_add() { if [ -z "$$FEATURES" ]; then FEATURES="$$1"; else FEATURES="$$FEATURES,$$1"; fi; }; \
	if [ -n "$$VOSK_LIB" ] && [ -d "$(MODELS_DIR)/vosk/$(VOSK_MODEL_NAME)" ]; then _add vosk-stt; fi; \
	if [ -f "$$WHISPER_MODEL" ]; then \
		if [ "$$OS" = "Darwin" ]; then _add whisper-stt-metal; else _add whisper-stt; fi; \
	fi; \
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then _add mlx-tts; fi; \
	echo ""; \
	echo "=== marginalia-tui ==="; \
	echo "  platform: $$OS $$ARCH"; \
	echo "  config:   $(TUI_TOML)"; \
	echo "  features: $${FEATURES:-none}"; \
	echo "========================"; \
	echo ""; \
	VOSK_PATH=$(VOSK_LIB_DIR) \
	LIBRARY_PATH=$(VOSK_LIB_DIR):$(KOKORO_ASSETS_DIR)/lib:$$LIBRARY_PATH \
	LD_LIBRARY_PATH=$(VOSK_LIB_DIR):$(KOKORO_ASSETS_DIR)/lib:$$LD_LIBRARY_PATH \
	DYLD_LIBRARY_PATH=$(VOSK_LIB_DIR):$(KOKORO_ASSETS_DIR)/lib:$$DYLD_LIBRARY_PATH \
	cargo run --release --manifest-path apps/tui-rs/Cargo.toml \
		$$([ -n "$$FEATURES" ] && echo "--features $$FEATURES")

beta-test:
	cargo test

beta-doctor:
	cargo run -p marginalia-devtools -- kokoro-doctor $(KOKORO_ASSETS_DIR)

# ---------------------------------------------------------------------------
# Clean
# ---------------------------------------------------------------------------

clean:
	rm -rf target/
	rm -rf benchmark/macos-apple-silicon/rust/target/
	rm -rf benchmark/macos-apple-silicon/rust-voice/target/
	rm -rf benchmark/macos-apple-silicon/rust-mlx/target/
	rm -rf .bench-venv/
	rm -rf .marginalia/
	rm -rf .marginalia-tts-cache/
	rm -f apps/tui-rs/marginalia.toml
	rm -f *.log *.db *.db-shm *.db-wal
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	@echo "All build artifacts, caches, and session data cleaned."

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
	model_path = "$$ROOT_DIR/models/stt/vosk/$(VOSK_MODEL_NAME)"
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
