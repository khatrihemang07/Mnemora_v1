# Mnemora tasks. Every build runs with CARGO_BUILD_JOBS=2 (8 GB dev machine) and every
# long-running command goes through scripts/with-env.sh, which loads .env.local and
# kills the whole child process tree on exit or Ctrl-C.

SHELL := /bin/bash
.DEFAULT_GOAL := help
RUN := ./scripts/with-env.sh
SANDBOX_ARGS := --config src-tauri/tauri.sandbox.conf.json --features sandbox
DEV_PORT := 47821
# Tauri merges build.features from the sandbox config too; passing it explicitly as well
# means a CLI that ignored the config could still never start Sandbox on Production data
# (the app also refuses to start when identifier and build Environment disagree).

.PHONY: help dev sandbox build android-dev android test lint fmt icons bindings trim clean-heavy setup

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  \033[1m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

define require_free_port
	@if lsof -iTCP:$(DEV_PORT) -sTCP:LISTEN >/dev/null 2>&1; then \
	  echo "Port $(DEV_PORT) is already in use: a dev server is probably running (see 'pgrep -fl vite'). Stop it first."; \
	  exit 1; \
	fi
endef

dev: ## Hot reload on Production data (DB is backed up before migrations)
	$(require_free_port)
	$(RUN) cargo tauri dev

sandbox: ## Hot reload on separate Sandbox data with the SANDBOX badge
	$(require_free_port)
	$(RUN) cargo tauri dev $(SANDBOX_ARGS)

build: ## Release build for this machine (.app/.dmg), signed if setup-signing was run
	@eval "$$(./scripts/signing-env.sh mac)"; $(RUN) cargo tauri build

android-dev: ## Hot reload on a USB-connected phone (Sandbox data)
	$(require_free_port)
	@export MNEMORA_ANDROID_SANDBOX=1; \
	  trap 'adb reverse --remove tcp:$(DEV_PORT) >/dev/null 2>&1 || true' EXIT; \
	  $(RUN) adb reverse tcp:$(DEV_PORT) tcp:$(DEV_PORT) && \
	  $(RUN) cargo tauri android dev --features sandbox

android: ## Signed arm64 release APK, installed on the connected phone
	@set -e; unset MNEMORA_ANDROID_SANDBOX; eval "$$(./scripts/signing-env.sh android)"; \
	$(RUN) cargo tauri android build --target aarch64 --apk; \
	apk=$$(find src-tauri/gen/android/app/build/outputs/apk -name '*release*.apk' ! -name '*unsigned*' -print0 | xargs -0 ls -t | head -1); \
	test -n "$$apk" || { echo "no signed release APK found (run make setup?)"; exit 1; }; \
	echo "Installing $$apk"; \
	$(RUN) adb install -r "$$apk"

test: ## Rust + frontend tests
	$(RUN) cargo test --workspace
	$(RUN) pnpm vitest run --passWithNoTests

lint: ## clippy -D warnings, rustfmt check, raw-colour check, Biome, svelte-check
	$(RUN) cargo clippy --workspace --all-targets -- -D warnings
	$(RUN) cargo fmt --all --check
	./scripts/check-colors.sh
	$(RUN) pnpm biome check .
	$(RUN) pnpm svelte-check --tsconfig ./tsconfig.json

fmt: ## Format Rust and frontend code
	$(RUN) cargo fmt --all
	$(RUN) pnpm biome check --write .

icons: ## Regenerate all app icons + favicon from src-tauri/icons/source/*.svg
	./scripts/make-icons.sh

bindings: ## Regenerate src/lib/ipc/bindings.ts without launching the app
	$(RUN) cargo test -p mnemora-app regenerate_bindings -- --ignored

trim: ## Free disk, keeping compiled deps: incremental dirs, docs, Gradle outputs
	@before=$$(du -sk target 2>/dev/null | cut -f1); \
	rm -rf target/*/incremental target/*/*/incremental target/*/doc \
	  src-tauri/gen/android/build src-tauri/gen/android/app/build; \
	after=$$(du -sk target 2>/dev/null | cut -f1); \
	echo "target/: $$((before/1024)) MB -> $$((after/1024)) MB"

clean-heavy: ## Delete target/, node_modules and Android build dirs (asks first)
	@read -r -p "Delete target/, node_modules/ and src-tauri/gen/android build dirs? [y/N] " ans; \
	if [ "$$ans" = "y" ] || [ "$$ans" = "Y" ]; then \
	  rm -rf target node_modules dist \
	    src-tauri/gen/android/.gradle src-tauri/gen/android/build src-tauri/gen/android/app/build; \
	  echo "Removed. Run 'pnpm install' before the next build."; \
	else echo "Cancelled."; fi

setup: ## One-time Android toolchain and signing setup (modifies your system; read the scripts first)
	./scripts/setup-android.sh
	./scripts/setup-signing.sh
