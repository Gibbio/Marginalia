KOKORO_ASSETS_DIR  ?= models/tts/kokoro
MLX_MODEL_DIR      ?= models/tts/mlx
MLX_HF_REPO        ?= prince-canuma/Kokoro-82M

# ---------------------------------------------------------------------------
# Language detection
# Reads the system locale to pick default TTS voice and STT language.
# Override from the command line: make tui-rs MARGINALIA_LANG=it
# Default: en (English).
# ---------------------------------------------------------------------------
ifndef MARGINALIA_LANG
  ifeq ($(shell uname -s),Darwin)
    MARGINALIA_LANG := $(shell defaults read -g AppleLocale 2>/dev/null \
      | cut -d_ -f1 | cut -d@ -f1 | cut -d- -f1 \
      | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z')
  endif
  ifeq ($(MARGINALIA_LANG),)
    MARGINALIA_LANG := $(shell echo "$(LANG)" \
      | cut -d_ -f1 | cut -d. -f1 \
      | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z')
  endif
  ifeq ($(MARGINALIA_LANG),)
    MARGINALIA_LANG := en
  endif
endif

# ---------------------------------------------------------------------------
# Voice table (source: hexgrad/Kokoro-82M/VOICES.md)
# Format: MLX_VOICE_DEFAULT_<lang> = best voice for that language
#         MLX_VOICES_<lang>        = all voices to download (space-separated)
#
# English voices (af_*, am_*, bf_*, bm_*) are embedded in the binary —
# no download needed; MLX_VOICES_en is intentionally empty.
# All other voices must be fetched from HuggingFace via bootstrap-mlx.
# ---------------------------------------------------------------------------
MLX_VOICE_DEFAULT_en := af_heart
MLX_VOICES_en        :=

MLX_VOICE_DEFAULT_it := if_sara
MLX_VOICES_it        := if_sara im_nicola

MLX_VOICE_DEFAULT_ja := jf_alpha
MLX_VOICES_ja        := jf_alpha jf_gongitsune jf_nezumi jf_tebukuro jm_kumo

MLX_VOICE_DEFAULT_zh := zf_xiaoxiao
MLX_VOICES_zh        := zf_xiaobei zf_xiaoni zf_xiaoxiao zf_xiaoyi zm_yunjian zm_yunxi zm_yunxia zm_yunyang

MLX_VOICE_DEFAULT_es := ef_dora
MLX_VOICES_es        := ef_dora em_alex em_santa

MLX_VOICE_DEFAULT_fr := ff_siwis
MLX_VOICES_fr        := ff_siwis

MLX_VOICE_DEFAULT_hi := hf_alpha
MLX_VOICES_hi        := hf_alpha hf_beta hm_omega hm_psi

MLX_VOICE_DEFAULT_pt := pf_dora
MLX_VOICES_pt        := pf_dora pm_alex pm_santa

# Resolve for the detected language; fall back to English if unsupported.
_VOICE_DEFAULT_RESOLVED := $(MLX_VOICE_DEFAULT_$(MARGINALIA_LANG))
ifeq ($(_VOICE_DEFAULT_RESOLVED),)
  _VOICE_DEFAULT_RESOLVED := $(MLX_VOICE_DEFAULT_en)
endif

_VOICES_RESOLVED := $(MLX_VOICES_$(MARGINALIA_LANG))
# (no fallback needed for voices: unsupported lang → empty → no download)

MLX_VOICE_DEFAULT ?= $(_VOICE_DEFAULT_RESOLVED)
MLX_VOICES        ?= $(_VOICES_RESOLVED)
VOSK_MODEL_URL     ?= https://alphacephei.com/vosk/models/vosk-model-small-it-0.22.zip
VOSK_MODEL_NAME    ?= vosk-model-small-it-0.22
MODELS_DIR         ?= models/stt
VOSK_LIB_VERSION   ?= 0.3.42
VOSK_LIB_DIR       ?= models/stt/vosk
ORT_VERSION        ?= 1.24.4
WHISPER_MODEL_DIR  ?= models/stt/whisper
WHISPER_MODEL_NAME ?= ggml-small.bin
WHISPER_MODEL_URL  ?= https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$(WHISPER_MODEL_NAME)

.PHONY: \
	bootstrap-beta bootstrap-kokoro bootstrap-ort bootstrap-vosk bootstrap-vosk-lib \
	bootstrap-mlx _kokoro-hf-cli _kokoro-curl \
	check-deps tui-rs beta-test beta-doctor \
	setup bootstrap bootstrap-runtime-deps bootstrap-providers \
	bootstrap-kokoro-python bootstrap-whisper bootstrap-system-deps setup-config \
	format lint test smoke run-cli-help doctor \
	clean clean-alpha clean-session

# ---------------------------------------------------------------------------
# Beta — provider bootstrapping
# ---------------------------------------------------------------------------

# Download all Beta providers in one shot.
bootstrap-beta: bootstrap-kokoro bootstrap-vosk bootstrap-vosk-lib bootstrap-whisper bootstrap-mlx
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

# Download MLX model weights and Italian voices to a local directory (macOS arm64 only).
# Stores assets in $(MLX_MODEL_DIR) so the TUI never needs to reach HuggingFace at runtime.
# The runtime picks up the local path via [mlx] model = "$(MLX_MODEL_DIR)" in marginalia.toml.
bootstrap-mlx:
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	if [ "$$OS" != "Darwin" ] || [ "$$ARCH" != "arm64" ]; then \
		echo "bootstrap-mlx: MLX is macOS arm64 only — skipping."; \
		exit 0; \
	fi; \
	echo "Downloading MLX model assets ($(MLX_HF_REPO))..."; \
	mkdir -p $(MLX_MODEL_DIR)/voices; \
	if command -v hf >/dev/null 2>&1; then HF_CLI=hf; \
	elif command -v huggingface-cli >/dev/null 2>&1; then HF_CLI=huggingface-cli; \
	else HF_CLI=""; fi; \
	WEIGHTS="$(MLX_MODEL_DIR)/kokoro-v1_0.safetensors"; \
	if [ -f "$$WEIGHTS" ]; then \
		echo "  kokoro-v1_0.safetensors already present, skipping."; \
	elif [ -n "$$HF_CLI" ]; then \
		$$HF_CLI download $(MLX_HF_REPO) kokoro-v1_0.safetensors --local-dir $(MLX_MODEL_DIR); \
	else \
		echo "  Downloading via curl..."; \
		curl -fL --progress-bar \
			-o "$$WEIGHTS" \
			"https://huggingface.co/$(MLX_HF_REPO)/resolve/main/kokoro-v1_0.safetensors" \
			|| { echo "Failed to download model weights"; exit 1; }; \
	fi; \
	for VOICE in $(MLX_VOICES); do \
		DEST="$(MLX_MODEL_DIR)/voices/$${VOICE}.safetensors"; \
		if [ -f "$$DEST" ]; then \
			echo "  voices/$${VOICE}.safetensors already present, skipping."; \
		elif [ -n "$$HF_CLI" ]; then \
			$$HF_CLI download $(MLX_HF_REPO) "voices/$${VOICE}.safetensors" --local-dir $(MLX_MODEL_DIR); \
		else \
			echo "  Downloading voices/$${VOICE}.safetensors via curl..."; \
			curl -fL --progress-bar \
				-o "$$DEST" \
				"https://huggingface.co/$(MLX_HF_REPO)/resolve/main/voices/$${VOICE}.safetensors" \
				|| { echo "  $$VOICE not available, skipping."; rm -f "$$DEST"; }; \
		fi; \
	done; \
	echo ""; \
	echo "MLX assets ready at $(MLX_MODEL_DIR)/."

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
		TTS_SECTION='# Kokoro via MLX Metal GPU (assets at $(MLX_MODEL_DIR))\n[mlx]\nmodel = "$(MLX_MODEL_DIR)"\nvoice = "$(MLX_VOICE_DEFAULT)"'; \
	elif [ -d "$(KOKORO_ASSETS_DIR)" ]; then \
		TTS_SECTION='# Kokoro via ONNX Runtime CPU\n[kokoro]\nassets_root = "$(KOKORO_ASSETS_DIR)"\nphonemizer_program = "espeak-ng"\nphonemizer_args = ["-v", "it", "--ipa", "-q"]'; \
	else \
		TTS_SECTION='# No TTS configured. Run: make bootstrap-kokoro'; \
	fi; \
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then \
		DEFAULT_ENGINE="apple"; \
	else \
		DEFAULT_ENGINE="whisper"; \
	fi; \
	STT_SECTION="[stt]\nengine   = \"$$DEFAULT_ENGINE\"     # \"apple\" or \"whisper\"\nlanguage = \"$(MARGINALIA_LANG)\"        # ISO (\"it\") or BCP-47 (\"it-IT\"); auto-converted per engine\ndebug    = true        # show raw transcript in the Log pane"; \
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then \
		STT_SECTION="$$STT_SECTION\n\n# Apple-engine settings.\n# Requires: System Settings → Keyboard → Dictation → ON.\n# NOTE: Apple dictation is not yet implemented — pick engine = \"whisper\" if\n# you need /note. See NEXT.md (\"Apple STT dictation mode\").\n[stt.apple]"; \
	fi; \
	if [ -f "$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)" ]; then \
		STT_SECTION="$$STT_SECTION\n\n# Whisper-engine settings.\n[stt.whisper]\nmodel_path = \"$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME)\""; \
	else \
		STT_SECTION="$$STT_SECTION\n\n# Whisper-engine settings.\n# [stt.whisper]\n# model_path = \"models/stt/whisper/ggml-small.bin\""; \
	fi; \
	STT_SECTION="$$STT_SECTION\n\n# Tuning profile for SHORT utterances (voice commands).\n# Defaults: silence_timeout = 0.8, max_record_seconds = 4, speech_threshold = 500\n[stt.commands]\n# silence_timeout    = 0.8\n# max_record_seconds = 4\n# speech_threshold   = 500"; \
	STT_SECTION="$$STT_SECTION\n\n# Tuning profile for LONG utterances (note dictation via /note).\n# Defaults: silence_timeout = 1.5, max_record_seconds = 60, speech_threshold = 500\n[stt.dictation]\n# silence_timeout    = 1.5\n# max_record_seconds = 60\n# speech_threshold   = 500"; \
	sed -e "s|__PLATFORM__|$$PLATFORM|" \
	    -e "s|__DATE__|$$DATE|" \
	    -e "s|__TTS_SECTION__|$$TTS_SECTION|" \
	    -e "s|__STT_SECTION__|$$STT_SECTION|" \
	    $(TUI_TEMPLATE) > $(TUI_TOML); \
	echo "Generated $(TUI_TOML) for $$PLATFORM"

# ---------------------------------------------------------------------------
# Pre-build dependency check
# ---------------------------------------------------------------------------

# Verify that all tools required to compile tui-rs are present.
# Runs automatically before tui-rs; also callable standalone: make check-deps
check-deps:
	@echo "=== Pre-build dependency check ==="
	@ERRORS=0; \
	OS=$$(uname -s); ARCH=$$(uname -m); \
	\
	if command -v cargo >/dev/null 2>&1; then \
		echo "  [OK] $$(cargo --version 2>&1 | head -1)"; \
	else \
		echo "  [MISSING] cargo/rustc — install via https://rustup.rs"; \
		ERRORS=$$((ERRORS+1)); \
	fi; \
	\
	if command -v cmake >/dev/null 2>&1; then \
		echo "  [OK] $$(cmake --version | head -1)"; \
	else \
		echo "  [MISSING] cmake — brew install cmake"; \
		ERRORS=$$((ERRORS+1)); \
	fi; \
	\
	if command -v espeak-ng >/dev/null 2>&1; then \
		echo "  [OK] $$(espeak-ng --version 2>&1 | head -1)"; \
	else \
		echo "  [MISSING] espeak-ng — brew install espeak-ng"; \
		ERRORS=$$((ERRORS+1)); \
	fi; \
	\
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then \
		XCODE_PATH=$$(xcode-select -p 2>/dev/null || true); \
		if echo "$$XCODE_PATH" | grep -q "Xcode.app"; then \
			echo "  [OK] Xcode at $$XCODE_PATH"; \
		else \
			echo "  [MISSING] Xcode.app — MLX Metal requires the full Xcode, not just CLT."; \
			echo "             Install from the App Store, then: sudo xcode-select -s /Applications/Xcode.app"; \
			ERRORS=$$((ERRORS+1)); \
		fi; \
		if xcrun -sdk macosx metal --version >/dev/null 2>&1; then \
			echo "  [OK] Metal compiler ($$(xcrun -sdk macosx metal --version 2>&1 | head -1))"; \
		else \
			echo "  [MISSING] Metal compiler — open Xcode, go to Settings → Platforms and install macOS."; \
			ERRORS=$$((ERRORS+1)); \
		fi; \
		if command -v swiftc >/dev/null 2>&1; then \
			echo "  [OK] $$(swiftc --version 2>&1 | head -1)"; \
		else \
			echo "  [MISSING] swiftc — install Xcode from the App Store"; \
			ERRORS=$$((ERRORS+1)); \
		fi; \
		if [ -f "$(MLX_MODEL_DIR)/kokoro-v1_0.safetensors" ]; then \
			echo "  [OK] MLX model weights ($(MLX_MODEL_DIR)/kokoro-v1_0.safetensors)"; \
		else \
			echo "  [MISSING] MLX model weights — run: make bootstrap-mlx"; \
			ERRORS=$$((ERRORS+1)); \
		fi; \
		for VOICE in $(MLX_VOICES); do \
			if [ -f "$(MLX_MODEL_DIR)/voices/$${VOICE}.safetensors" ]; then \
				echo "  [OK] MLX voice $${VOICE}"; \
			else \
				echo "  [MISSING] MLX voice '$${VOICE}' — run: make bootstrap-mlx"; \
				ERRORS=$$((ERRORS+1)); \
			fi; \
		done; \
	fi; \
	\
	echo ""; \
	if [ "$$ERRORS" -gt 0 ]; then \
		echo "  $$ERRORS missing dependency/dependencies. Install them and retry."; \
		echo ""; \
		exit 1; \
	else \
		echo "  All dependencies found."; \
		echo ""; \
	fi

tui-rs: bootstrap-mlx check-deps
	@$(MAKE) --no-print-directory $(TUI_TOML)
	@OS=$$(uname -s); ARCH=$$(uname -m); \
	WHISPER_MODEL=$(WHISPER_MODEL_DIR)/$(WHISPER_MODEL_NAME); \
	FEATURES=""; \
	_add() { if [ -z "$$FEATURES" ]; then FEATURES="$$1"; else FEATURES="$$FEATURES,$$1"; fi; }; \
	if [ -f "$$WHISPER_MODEL" ]; then \
		if [ "$$OS" = "Darwin" ]; then _add whisper-stt-metal; else _add whisper-stt; fi; \
	fi; \
	if [ "$$OS" = "Darwin" ] && [ "$$ARCH" = "arm64" ]; then \
		_add mlx-tts; _add apple-stt; \
	fi; \
	echo ""; \
	echo "=== marginalia-tui ==="; \
	echo "  platform: $$OS $$ARCH"; \
	echo "  config:   $(TUI_TOML)"; \
	echo "  features: $${FEATURES:-none}"; \
	if echo "$$FEATURES" | grep -q "apple-stt"; then \
		echo "  note:     Apple STT requires: System Settings → Keyboard → Dictation → ON"; \
	fi; \
	echo "========================"; \
	echo ""; \
	LIBRARY_PATH=$(KOKORO_ASSETS_DIR)/lib:$$LIBRARY_PATH \
	LD_LIBRARY_PATH=$(KOKORO_ASSETS_DIR)/lib:$$LD_LIBRARY_PATH \
	DYLD_LIBRARY_PATH=$(KOKORO_ASSETS_DIR)/lib:$$DYLD_LIBRARY_PATH \
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
