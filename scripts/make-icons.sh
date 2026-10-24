#!/usr/bin/env bash
# Regenerates every app icon from the SVG masters in src-tauri/icons/source/. Run via `make icons`.
#
#   app-icon.svg            macOS/Linux/iOS + dev Dock icon (824px tile, transparent 100px margin)
#   app-icon-fullbleed.svg  Windows .ico/Square logos + web favicon (no OS margin)
#   Android                 vector adaptive icon (gradient background + spiral foreground),
#                           written straight into the committed gen/android project
#
# Rendering uses `cargo tauri icon` (built-in resvg). Don't use qlmanage: it paints an opaque
# white background behind the transparent margin.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT_DIR/src-tauri/icons/source"
ICONS="$ROOT_DIR/src-tauri/icons"
RES="$ROOT_DIR/src-tauri/gen/android/app/src/main/res"
PUBLIC="$ROOT_DIR/public"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "==> Desktop + iOS from app-icon.svg"
cargo tauri icon "$SRC/app-icon.svg" -o "$TMP/main" >/dev/null
for f in icon.icns icon.png 32x32.png 64x64.png 128x128.png 128x128@2x.png; do
  cp "$TMP/main/$f" "$ICONS/$f"
done
rm -rf "$ICONS/ios" && cp -R "$TMP/main/ios" "$ICONS/ios"
# Android comes from the vector drawables below, not tauri's PNGs (they'd sit on white).
rm -rf "$ICONS/android"

echo "==> Sandbox (Mac) from app-icon-sandbox.svg"
cargo tauri icon "$SRC/app-icon-sandbox.svg" -o "$TMP/sandbox" >/dev/null
mkdir -p "$ICONS/sandbox"
for f in icon.icns icon.png 32x32.png 128x128.png 128x128@2x.png; do
  cp "$TMP/sandbox/$f" "$ICONS/sandbox/$f"
done

echo "==> Windows + favicon from app-icon-fullbleed.svg"
cargo tauri icon "$SRC/app-icon-fullbleed.svg" -o "$TMP/full" >/dev/null
cp "$TMP/full/icon.ico" "$TMP/full/StoreLogo.png" "$ICONS/"
cp "$TMP"/full/Square*Logo.png "$ICONS/"
cargo tauri icon "$SRC/app-icon-fullbleed.svg" -o "$TMP/touch" -p 180 -p 32 >/dev/null
mkdir -p "$PUBLIC"
cp "$SRC/app-icon-fullbleed.svg" "$PUBLIC/favicon.svg"
cp "$TMP/touch/32x32.png" "$PUBLIC/favicon-32.png"
cp "$TMP/touch/180x180.png" "$PUBLIC/apple-touch-icon.png"

echo "==> Android adaptive icon"
if [ -d "$RES" ]; then
  # Drop Tauri/Android Studio defaults (Tauri logo PNGs, robot foreground, green grid).
  rm -f "$RES"/mipmap-*dpi/ic_launcher*.png "$RES/drawable-v24/ic_launcher_foreground.xml"
  rmdir "$RES/drawable-v24" "$RES"/mipmap-*dpi 2>/dev/null || true
  mkdir -p "$RES/drawable" "$RES/mipmap-anydpi-v26"
  cp "$SRC/android/ic_launcher_background.xml" "$RES/drawable/"
  cp "$SRC/android/ic_launcher_foreground.xml" "$RES/drawable/"
  cp "$SRC/android/ic_launcher.xml" "$RES/mipmap-anydpi-v26/ic_launcher.xml"
  cp "$SRC/android/ic_launcher.xml" "$RES/mipmap-anydpi-v26/ic_launcher_round.xml"
  # The Sandbox app's own icon (build.gradle.kts picks it through the appIcon placeholder).
  cp "$SRC/android/ic_launcher_sandbox_background.xml" "$RES/drawable/"
  cp "$SRC/android/ic_launcher_sandbox.xml" "$RES/mipmap-anydpi-v26/ic_launcher_sandbox.xml"
else
  echo "    $RES not found; run the Android setup first, then re-run make icons"
fi

echo "==> Done. Rebuild to see the new icons (macOS may cache the old Dock icon until relaunch)."
