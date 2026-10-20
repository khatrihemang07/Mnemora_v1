#!/usr/bin/env bash
# Prints `export` lines for the signing secrets stored by setup-signing.sh, for use as
#   eval "$(scripts/signing-env.sh mac|android)"
# Secrets live in the macOS Keychain, never on disk. Prints nothing if not set up.
set -uo pipefail

KEYCHAIN="$HOME/Library/Keychains/mnemora-signing.keychain-db"
CERT_NAME="Mnemora Dev"

secret() { security find-generic-password -s "$1" -a "$USER" -w 2>/dev/null; }

case "${1:-}" in
  mac)
    if [ -f "$KEYCHAIN" ] && pass=$(secret mnemora-signing-keychain); then
      security unlock-keychain -p "$pass" "$KEYCHAIN" 2>/dev/null || true
      printf 'export APPLE_SIGNING_IDENTITY=%q\n' "$CERT_NAME"
    fi
    ;;
  android)
    if pass=$(secret mnemora-android-keystore); then
      printf 'export MNEMORA_KEYSTORE_PATH=%q\n' "$HOME/.mnemora/release.keystore"
      printf 'export MNEMORA_KEYSTORE_PASSWORD=%q\n' "$pass"
      printf 'export MNEMORA_KEY_ALIAS=%q\n' "mnemora"
    fi
    ;;
  *)
    echo "usage: signing-env.sh mac|android" >&2
    exit 2
    ;;
esac
