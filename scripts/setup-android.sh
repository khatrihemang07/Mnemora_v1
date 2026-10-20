#!/usr/bin/env bash
# One-time, minimal Android toolchain setup for an 8 GB Mac. Safe to re-run.
#
# What it changes (and nothing else):
#   - installs the latest stable NDK via the Homebrew sdkmanager
#   - removes the emulator and build-tools 35.0.0 (we test on a real phone; saves ~2 GB)
#   - adds the aarch64-linux-android Rust target
#   - writes a git-ignored .env.local with ANDROID_HOME, NDK_HOME, JAVA_HOME, GRADLE_OPTS
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

echo "==> Rust Android target (arm64 only)"
rustup target add aarch64-linux-android

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
  for kv in "org.gradle.daemon=false" "org.gradle.parallel=false" "org.gradle.jvmargs=-Xmx1536m"; do
    key="${kv%%=*}"
    if grep -q "^$key=" "$GRADLE_PROPS"; then
      sed -i '' "s|^$key=.*|$kv|" "$GRADLE_PROPS"
    else
      echo "$kv" >>"$GRADLE_PROPS"
    fi
  done
fi

echo "==> Done. Plug in your phone, enable USB debugging, then check: adb devices"
