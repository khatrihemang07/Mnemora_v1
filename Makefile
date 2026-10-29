# Mnemora tasks. Every build runs with CARGO_BUILD_JOBS=2 (8 GB dev machine) and every
# long-running command goes through scripts/with-env.sh, which loads .env.local and
# kills the whole child process tree on exit or Ctrl-C.

SHELL := /bin/bash
.DEFAULT_GOAL := help
RUN := ./scripts/with-env.sh
# dev, sandbox and android-dev run scripts/dev.mjs: Vite plus a key menu. ARGS passes flags to
# it, e.g. make dev ARGS="--mac --android". The dev port, its check and the command lines live
# in scripts/dev/ (port 47821 is also in vite.config.ts and src-tauri/tauri.conf.json).

.PHONY: help dev sandbox seed-sandbox build android-dev android android-sandbox android-test test lint fmt icons bindings trim clean-heavy setup

help: ## Show this help
	@awk 'BEGIN {FS = ":.*## "} /^[a-z-]+:.*## / {printf "  \033[1m%-12s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Vite + key menu (w web, z Mac, r relaunch, x stop, q quit); Production data
	$(RUN) node scripts/dev.mjs $(ARGS)

sandbox: ## Vite + key menu (z Mac, a Android), apps on separate Sandbox data with the SANDBOX badge
	$(RUN) node scripts/dev.mjs --sandbox $(ARGS)

seed-sandbox: ## Fill the Sandbox database with the recent-changes fixture, dated from today (quit the Sandbox app first; ARGS= flags in README)
	$(RUN) cargo run -p mnemora-app --features seed --example seed_sandbox -- $(ARGS)

build: ## Release build for this machine (.app/.dmg), signed if setup-signing was run
	@eval "$$(./scripts/signing-env.sh mac)"; $(RUN) cargo tauri build

android-dev: ## Vite + key menu, Android app on a USB-connected phone (Sandbox data)
	$(RUN) node scripts/dev.mjs --sandbox --android

android: ## Signed arm64 release APK, installed on the connected phone
	@set -e; unset MNEMORA_ANDROID_SANDBOX; eval "$$(./scripts/signing-env.sh android)"; \
	$(RUN) cargo tauri android build --target aarch64 --apk; \
	apk=$$(find src-tauri/gen/android/app/build/outputs/apk -name '*release*.apk' ! -name '*unsigned*' -print0 | xargs -0 ls -t | head -1); \
	test -n "$$apk" || { echo "no signed release APK found (run make setup?)"; exit 1; }; \
	echo "Installing $$apk"; \
	$(RUN) adb install -r "$$apk"

android-sandbox: ## Debug Sandbox APK (no hot reload, no dev server) installed on the phone
	@set -e; export MNEMORA_ANDROID_SANDBOX=1; \
	$(RUN) cargo tauri android build --debug --target aarch64 --apk --features sandbox; \
	apk=$$(find src-tauri/gen/android/app/build/outputs/apk -name '*debug*.apk' -print0 | xargs -0 ls -t | head -1); \
	test -n "$$apk" || { echo "no debug APK found"; exit 1; }; \
	echo "Installing $$apk"; \
	$(RUN) adb install -r "$$apk"

android-test: ## Kotlin unit tests (Todo widget logic); kept out of `make test` because Gradle is heavy
	$(RUN) bash -c 'cd src-tauri/gen/android && ./gradlew --max-workers=2 :app:testDebugUnitTest && ./gradlew --stop'

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
