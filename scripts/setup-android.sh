#!/usr/bin/env bash
# One-time, minimal Android toolchain setup for an 8 GB Mac. Safe to re-run.
#
# What it changes:
#   - installs the latest stable NDK via the Homebrew sdkmanager
#   - prunes the NDK to an arm64-only Rust toolchain (deletes files inside the NDK folder)
#   - removes the emulator and build-tools 35.0.0 (we test on a real phone; saves ~2 GB)
#   - adds the aarch64-linux-android Rust target and removes the other Android targets
#   - writes a git-ignored .env.local with ANDROID_HOME, NDK_HOME, JAVA_HOME, GRADLE_OPTS
#   - sets memory limits in src-tauri/gen/android/gradle.properties (if the file exists)
# It makes no change to your shell profile or global Gradle config.
#
# Usage: ./scripts/setup-android.sh [--yes]
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT_DIR/.env.local"
BREW_PREFIX="$(brew --prefix)"
ANDROID_HOME="${ANDROID_HOME:-$BREW_PREFIX/share/android-commandlinetools}"
JAVA_HOME_DIR="$BREW_PREFIX/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home"
SDKMANAGER="$BREW_PREFIX/bin/sdkmanager"

[ -x "$SDKMANAGER" ] || { echo "sdkmanager not found: brew install --cask android-commandlinetools"; exit 1; }
[ -d "$JAVA_HOME_DIR" ] || { echo "OpenJDK 21 not found: brew install openjdk@21"; exit 1; }
export JAVA_HOME="$JAVA_HOME_DIR"

echo "==> Looking up the latest stable NDK"
NDK_PACKAGE="$("$SDKMANAGER" --sdk_root="$ANDROID_HOME" --list 2>/dev/null \
  | grep -Eo 'ndk;[0-9]+\.[0-9]+\.[0-9]+' | sort -uV | tail -1)"
[ -n "$NDK_PACKAGE" ] || { echo "could not determine the latest NDK"; exit 1; }
NDK_VERSION="${NDK_PACKAGE#ndk;}"

echo "    NDK to install : $NDK_PACKAGE"
echo "    To remove      : emulator, build-tools;35.0.0 (if present)"
echo "    ANDROID_HOME   : $ANDROID_HOME"
if [ "${1:-}" != "--yes" ]; then
  read -r -p "Proceed? [y/N] " answer
  [ "$answer" = "y" ] || [ "$answer" = "Y" ] || { echo "Cancelled."; exit 0; }
fi

echo "==> Installing NDK"
"$SDKMANAGER" --sdk_root="$ANDROID_HOME" "$NDK_PACKAGE"

echo "==> Removing unused packages"
for pkg in "emulator" "build-tools;35.0.0"; do
  dir="$ANDROID_HOME/${pkg//;//}"
  if [ -d "$dir" ]; then
    "$SDKMANAGER" --sdk_root="$ANDROID_HOME" --uninstall "$pkg"
  else
    echo "    $pkg not installed, skipping"
  fi
done

echo "==> Pruning the NDK to an arm64-only Rust toolchain (~2.9 GB -> ~0.5 GB)"
# Rust needs only the clang driver, lld, llvm-ar/strip/objcopy and the aarch64 sysroot. Everything
# else (debugger, IDE tools, profilers, other CPU targets, the Intel half of binaries) is dropped.
# If a build ever needs something removed, re-run this script after deleting the ndk folder.
NDK_DIR="$ANDROID_HOME/ndk/$NDK_VERSION"
LLVM="$NDK_DIR/toolchains/llvm/prebuilt/darwin-x86_64"
[ -n "$NDK_DIR" ] && [ -d "$NDK_DIR" ] || { echo "NDK folder not found: $NDK_DIR"; exit 1; }
[ -n "$LLVM" ] && [ -d "$LLVM" ] || { echo "NDK LLVM toolchain not found: $LLVM"; exit 1; }
rm -rf "$NDK_DIR/simpleperf" "$NDK_DIR/shader-tools" "$NDK_DIR/prebuilt" "$LLVM/python3"
(cd "$LLVM/bin" && rm -f clang-tidy clang-check clangd clang-scan-deps clang-format \
  clang-apply-replacements clang-include-fixer clang-doc clang-query clang-reorder-fields \
  clang-change-namespace clang-move clang-include-cleaner find-all-symbols modularize pp-trace \
  llvm-bolt perf2bolt llvm-boltdiff merge-fdata dsymutil llvm-dwp llvm-cfi-verify sancov lldb \
  lldb-argdumper lldb-server lldb-dap llvm-cov llvm-profdata llvm-bolt-heatmap llvm-exegesis)
(cd "$LLVM/lib" && rm -rf liblldb*.dylib libclang.dylib libclang-cpp.dylib LLVMPolly.so \
  libpython3*.dylib libsimpleperf_readelf.a python3*)
(cd "$LLVM/sysroot/usr/lib" && rm -rf arm-linux-androideabi i686-linux-android \
  riscv64-linux-android x86_64-linux-android)
for rt in "$LLVM"/lib/clang/*/lib/linux; do
  [ -d "$rt" ] || continue
  find "$rt" -maxdepth 1 -type f ! -name '*aarch64*' -delete
  find "$rt" -mindepth 1 -maxdepth 1 -type d ! -name aarch64 -exec rm -rf {} +
done
for f in "$LLVM"/bin/* "$LLVM"/lib/*.dylib; do
  [ -L "$f" ] && continue
  if lipo -info "$f" 2>/dev/null | grep -Eq "x86_64 arm64|arm64 x86_64"; then
    lipo -thin arm64 "$f" -output "$f.tmp" && mv "$f.tmp" "$f" && chmod +x "$f" || rm -f "$f.tmp"
  fi
done
echo "    NDK now $(du -sh "$NDK_DIR" | cut -f1)"

echo "==> Rust Android target (arm64 only)"
rustup target add aarch64-linux-android
# `cargo tauri android init` would add every Android target back; we only ever build arm64 and never re-run init.
rustup target remove armv7-linux-androideabi i686-linux-android x86_64-linux-android 2>/dev/null || true

echo "==> Writing $ENV_FILE"
BEGIN="# >>> mnemora android (managed by scripts/setup-android.sh) >>>"
END="# <<< mnemora android <<<"
touch "$ENV_FILE"
# Drop any previous managed block so re-runs stay idempotent.
TMP="$(mktemp)"
awk -v b="$BEGIN" -v e="$END" '$0==b{skip=1} !skip{print} $0==e{skip=0}' "$ENV_FILE" >"$TMP"
cat >>"$TMP" <<ENVEOF
$BEGIN
ANDROID_HOME="$ANDROID_HOME"
ANDROID_SDK_ROOT="$ANDROID_HOME"
NDK_HOME="$ANDROID_HOME/ndk/$NDK_VERSION"
JAVA_HOME="$JAVA_HOME_DIR"
# No Gradle daemon, no parallel workers, capped heap: stay inside 8 GB.
GRADLE_OPTS="-Dorg.gradle.daemon=false -Dorg.gradle.parallel=false -Xmx1536m"
$END
ENVEOF
mv "$TMP" "$ENV_FILE"
chmod 600 "$ENV_FILE"

# The generated Android project (created later by `cargo tauri android init`) gets the same
# limits in its own gradle.properties, if it already exists.
GRADLE_PROPS="$ROOT_DIR/src-tauri/gen/android/gradle.properties"
if [ -f "$GRADLE_PROPS" ]; then
  echo "==> Tuning $GRADLE_PROPS"
  # The generated file has no trailing newline; without this an append joins the last line.
  [ -z "$(tail -c1 "$GRADLE_PROPS")" ] || echo >>"$GRADLE_PROPS"
  for kv in "org.gradle.daemon=false" "org.gradle.parallel=false" \
    "org.gradle.jvmargs=-Xmx1536m -Dfile.encoding=UTF-8" "org.gradle.caching=false" \
    "kotlin.daemon.jvmargs=-Xmx768m" "kotlin.compiler.execution.strategy=in-process"; do
    key="${kv%%=*}"
    if grep -q "^$key=" "$GRADLE_PROPS"; then
      sed -i '' "s|^$key=.*|$kv|" "$GRADLE_PROPS"
    else
      echo "$kv" >>"$GRADLE_PROPS"
    fi
  done
fi

echo "==> Done. Plug in your phone, enable USB debugging, then check: adb devices"
