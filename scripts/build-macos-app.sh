#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/build/ModelRack.app"
CONTENTS="$APP/Contents"
MACOS="$CONTENTS/MacOS"
RESOURCES="$CONTENTS/Resources"
PROFILE="${MODELRACK_BUILD_PROFILE:-debug}"

case "${1:-}" in
  --release)
    PROFILE="release"
    ;;
  --debug|"")
    ;;
  *)
    printf 'usage: %s [--debug|--release]\n' "$0" >&2
    exit 2
    ;;
esac

if [[ "$PROFILE" != "debug" && "$PROFILE" != "release" ]]; then
  printf 'MODELRACK_BUILD_PROFILE must be debug or release, got %s\n' "$PROFILE" >&2
  exit 2
fi

VERSION="$(awk -F '"' '/^version =/ { print $2; exit }' "$ROOT/Cargo.toml")"
if [[ -z "$VERSION" ]]; then
  printf 'failed to read package version from Cargo.toml\n' >&2
  exit 1
fi

BUILD_ARGS=(--manifest-path "$ROOT/Cargo.toml")
TARGET_DIR="debug"
if [[ "$PROFILE" == "release" ]]; then
  BUILD_ARGS+=(--release)
  TARGET_DIR="release"
fi
BIN="$ROOT/target/$TARGET_DIR/modelrack"

cargo build "${BUILD_ARGS[@]}"

rm -rf "$APP"
mkdir -p "$MACOS" "$RESOURCES"
cp "$BIN" "$MACOS/modelrack"
cp "$ROOT/assets/AppIcon.icns" "$RESOURCES/AppIcon.icns"

cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleDisplayName</key>
  <string>ModelRack</string>
  <key>CFBundleExecutable</key>
  <string>modelrack</string>
  <key>CFBundleIconFile</key>
  <string>AppIcon</string>
  <key>CFBundleIdentifier</key>
  <string>dev.modelrack.ModelRack</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>ModelRack</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$VERSION</string>
  <key>CFBundleVersion</key>
  <string>$VERSION</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP" >/dev/null
printf 'Built %s (%s, v%s)\n' "$APP" "$PROFILE" "$VERSION"
