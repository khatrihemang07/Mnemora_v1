#!/usr/bin/env bash
# One-time creation of Mnemora's locally generated signing identities. There is no Apple
# Developer or Play Store account; these keep dev/release builds consistently signed.
#
# macOS : dedicated keychain + self-signed code-signing certificate "Mnemora Dev".
# Android: release keystore at ~/.mnemora/release.keystore.
#
# Safe to re-run: nothing that already exists is touched. Re-creating the Android keystore
# would make every installed copy un-upgradeable, so this script never does it.
#
# Secrets (keychain and keystore passwords) are stored in your login Keychain and read back
# into env vars at build time by scripts/signing-env.sh. No password is written to disk
# unencrypted and nothing secret goes in the repo.
#
# Usage: ./scripts/setup-signing.sh
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$HOME/.mnemora"

CERT_NAME="Mnemora Dev"
KEYCHAIN="$HOME/Library/Keychains/mnemora-signing.keychain-db"
KEYCHAIN_SERVICE="mnemora-signing-keychain"

KEYSTORE_PATH="$APP_DIR/release.keystore"
KEYSTORE_SERVICE="mnemora-android-keystore"
KEY_ALIAS="mnemora"
# Where the (gitignored) Gradle-side pointer lives. It holds no password.
KEYSTORE_PROPERTIES="$ROOT_DIR/src-tauri/gen/android/keystore.properties"

# keytool must come from the same JDK the Android build uses.
JAVA_HOME_DIR="$(brew --prefix 2>/dev/null)/opt/openjdk@21/libexec/openjdk.jdk/Contents/Home"
if [ -d "$JAVA_HOME_DIR" ]; then
  export JAVA_HOME="$JAVA_HOME_DIR"
  export PATH="$JAVA_HOME/bin:$PATH"
fi

# --- Keychain helpers (login keychain holds the secrets) ---------------------------------

secret_get() { security find-generic-password -s "$1" -a "$USER" -w 2>/dev/null; }
secret_set() { security add-generic-password -U -s "$1" -a "$USER" -w "$2" -T /usr/bin/security; }

# ---------------------------------------------------------------------------------------
# macOS: keychain + certificate (each created only if missing)
# ---------------------------------------------------------------------------------------

if [ "$(uname)" = "Darwin" ]; then
  echo "==> macOS: signing keychain"
  if [ -f "$KEYCHAIN" ] && KEYCHAIN_PASS="$(secret_get "$KEYCHAIN_SERVICE")"; then
    echo "    $KEYCHAIN already exists, keeping it"
    security unlock-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN"
  else
    if [ -f "$KEYCHAIN" ]; then
      echo "    $KEYCHAIN exists but its password is not in your login Keychain."
      echo "    Delete it yourself (security delete-keychain) and re-run to recreate."
      exit 1
    fi
    KEYCHAIN_PASS="$(openssl rand -base64 24)"
    security create-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN"
    secret_set "$KEYCHAIN_SERVICE" "$KEYCHAIN_PASS"
    security unlock-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN"
    security set-keychain-settings -u "$KEYCHAIN" # never auto-lock
    echo "    created $KEYCHAIN"
  fi

  # Make codesign find the identity without -k, without dropping the user's other keychains.
  if ! security list-keychains -d user | grep -q "mnemora-signing"; then
    EXISTING="$(security list-keychains -d user | tr -d '"' | tr '\n' ' ')"
    # shellcheck disable=SC2086
    security list-keychains -d user -s $EXISTING "$KEYCHAIN"
  fi

  echo "==> macOS: code-signing certificate"
  if security find-certificate -c "$CERT_NAME" "$KEYCHAIN" >/dev/null 2>&1; then
    echo "    '$CERT_NAME' already present, keeping it"
  else
    WORK_DIR="$(mktemp -d)"
    trap 'rm -rf "$WORK_DIR"' EXIT
    # Apple's LibreSSL: Homebrew OpenSSL 3 writes PKCS12 that `security import` rejects.
    /usr/bin/openssl req -x509 -newkey rsa:2048 \
      -keyout "$WORK_DIR/key.pem" -out "$WORK_DIR/cert.pem" \
      -days 3650 -nodes -subj "/CN=$CERT_NAME" \
      -addext "keyUsage=critical,digitalSignature" \
      -addext "extendedKeyUsage=codeSigning" 2>/dev/null
    /usr/bin/openssl pkcs12 -export -out "$WORK_DIR/cert.p12" \
      -inkey "$WORK_DIR/key.pem" -in "$WORK_DIR/cert.pem" -passout pass:"$KEYCHAIN_PASS"
    security import "$WORK_DIR/cert.p12" -P "$KEYCHAIN_PASS" -T /usr/bin/codesign -k "$KEYCHAIN"
    # Without this every codesign run prompts for the keychain password.
    security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASS" "$KEYCHAIN" >/dev/null
    echo "    '$CERT_NAME' installed"
    echo "    (self-signed identities show up in 'security find-identity -p codesigning'"
    echo "    without -v; codesign accepts them)"
  fi
fi

# ---------------------------------------------------------------------------------------
# Android: release keystore (never overwritten)
# ---------------------------------------------------------------------------------------

echo "==> Android: release keystore"
command -v keytool >/dev/null || { echo "keytool not found: brew install openjdk@21"; exit 1; }

if [ -f "$KEYSTORE_PATH" ]; then
  echo "    $KEYSTORE_PATH already exists, refusing to touch it."
  echo "    A new key can't sign upgrades over installed copies, and that can't be undone."
  if ! secret_get "$KEYSTORE_SERVICE" >/dev/null; then
    echo "    WARNING: its password is not in your login Keychain (service $KEYSTORE_SERVICE)."
    echo "    Add it yourself: security add-generic-password -s $KEYSTORE_SERVICE -a \$USER -w"
  fi
else
  mkdir -p "$APP_DIR"
  chmod 700 "$APP_DIR"
  # One password for store and key: PKCS12 keystores ignore a different key password.
  STORE_PASS="$(openssl rand -base64 24)"
  secret_set "$KEYSTORE_SERVICE" "$STORE_PASS"
  keytool -genkeypair \
    -keystore "$KEYSTORE_PATH" -alias "$KEY_ALIAS" \
    -keyalg RSA -keysize 2048 -validity 10000 \
    -storepass "$STORE_PASS" -keypass "$STORE_PASS" \
    -dname "CN=Mnemora, OU=Dev, O=Mnemora, C=US"
  chmod 600 "$KEYSTORE_PATH"
  echo "    created $KEYSTORE_PATH"

  echo
  echo "    The fingerprint identifies this key. Note it down:"
  keytool -list -v -keystore "$KEYSTORE_PATH" -alias "$KEY_ALIAS" -storepass "$STORE_PASS" \
    | grep -E "SHA256:|SHA-256:" | sed 's/^ */      /'
  echo
  echo "    LOSE THIS KEYSTORE AND INSTALLED APPS CAN NEVER BE UPDATED, ONLY REINSTALLED."
  read -r -p "    Folder to copy a backup into (USB drive, cloud folder), Enter to skip: " BACKUP_DIR
  if [ -n "$BACKUP_DIR" ]; then
    mkdir -p "$BACKUP_DIR"
    cp "$KEYSTORE_PATH" "$BACKUP_DIR/mnemora-release.keystore"
    echo "    copied to $BACKUP_DIR/mnemora-release.keystore"
  fi
  echo "    Keystore password is in your login Keychain. To store it in a password manager:"
  echo "      security find-generic-password -s $KEYSTORE_SERVICE -a \"\$USER\" -w"
fi

# The pointer file carries no password; Gradle reads it from MNEMORA_KEYSTORE_PASSWORD,
# which scripts/signing-env.sh exports from the Keychain at build time.
if [ ! -f "$KEYSTORE_PROPERTIES" ]; then
  mkdir -p "$(dirname "$KEYSTORE_PROPERTIES")"
  cat >"$KEYSTORE_PROPERTIES" <<PROPS
# Generated by scripts/setup-signing.sh. Gitignored. Contains no password.
# Gradle should take storePassword/keyPassword from \$MNEMORA_KEYSTORE_PASSWORD.
storeFile=$KEYSTORE_PATH
keyAlias=$KEY_ALIAS
PROPS
  chmod 600 "$KEYSTORE_PROPERTIES"
  echo "    wrote $KEYSTORE_PROPERTIES"
fi

echo "==> Done."
