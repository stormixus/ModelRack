#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$ROOT/build/ModelRack.app"
PROFILE="${MODELRACK_NOTARY_PROFILE:-}"
SIGN_IDENTITY="${MODELRACK_SIGN_IDENTITY:-}"
VOLNAME="${MODELRACK_DMG_VOLUME_NAME:-ModelRack}"
WINDOW_WIDTH=920
WINDOW_HEIGHT=440

usage() {
  cat >&2 <<USAGE
usage: $0 [--keychain-profile <profile>] [--sign-identity <developer-id-identity>]

Creates dist/ModelRack-v<version>-macos-<arch>.dmg with a polished Finder drag-to-Applications layout.
If --keychain-profile is supplied, the DMG is notarized and stapled.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keychain-profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --sign-identity)
      SIGN_IDENTITY="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      usage
      exit 2
      ;;
  esac
done

VERSION="$(awk -F '"' '/^version =/ { print $2; exit }' "$ROOT/Cargo.toml")"
ARCH="$(uname -m)"
DIST="$ROOT/dist"
WORK="$ROOT/build/dmg-work"
STAGE="$WORK/stage"
BG_SVG="$ROOT/assets/dmg/background.svg"
BG_PNG="$WORK/background.png"
RW_DMG="$WORK/ModelRack-rw.dmg"
FINAL_DMG="$DIST/ModelRack-v${VERSION}-macos-${ARCH}.dmg"
FINAL_SUM="$FINAL_DMG.sha256"

if [[ ! -d "$APP" ]]; then
  "$ROOT/scripts/build-macos-app.sh" --release
fi
if [[ ! -d "$APP" ]]; then
  printf 'Missing app bundle: %s\n' "$APP" >&2
  exit 1
fi
if [[ ! -f "$BG_SVG" ]]; then
  printf 'Missing DMG background: %s\n' "$BG_SVG" >&2
  exit 1
fi

mkdir -p "$DIST" "$WORK"
rm -rf "$STAGE" "$RW_DMG" "$FINAL_DMG" "$FINAL_SUM" "$BG_PNG"
mkdir -p "$STAGE/.background"

sips -s format png "$BG_SVG" --out "$BG_PNG" >/dev/null
cp "$BG_PNG" "$STAGE/.background/background.png"
cp -R "$APP" "$STAGE/ModelRack.app"
ln -s /Applications "$STAGE/Applications"

hdiutil create \
  -volname "$VOLNAME" \
  -srcfolder "$STAGE" \
  -ov \
  -format UDRW \
  -fs HFS+ \
  -size 180m \
  "$RW_DMG" >/dev/null

ATTACH_OUTPUT="$(hdiutil attach -readwrite -noverify -noautoopen "$RW_DMG")"
MOUNT_DIR="$(printf '%s\n' "$ATTACH_OUTPUT" | awk -F '\t' '/\/Volumes\// { print $NF; exit }')"
if [[ -z "$MOUNT_DIR" || ! -d "$MOUNT_DIR" ]]; then
  printf 'Failed to mount DMG. hdiutil output:\n%s\n' "$ATTACH_OUTPUT" >&2
  exit 1
fi

cleanup() {
  if mount | grep -q "on $MOUNT_DIR "; then
    hdiutil detach "$MOUNT_DIR" -quiet || hdiutil detach "$MOUNT_DIR" -force -quiet || true
  fi
}
trap cleanup EXIT

/usr/bin/osascript <<APPLESCRIPT
set volumeName to "$VOLNAME"
set windowWidth to $WINDOW_WIDTH
set windowHeight to $WINDOW_HEIGHT

tell application "Finder"
  tell disk volumeName
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set bounds of container window to {120, 120, 120 + windowWidth, 120 + windowHeight}
    set theViewOptions to icon view options of container window
    set arrangement of theViewOptions to not arranged
    set icon size of theViewOptions to 96
    set text size of theViewOptions to 12
    set label position of theViewOptions to bottom
    set background picture of theViewOptions to file ".background:background.png"
    set position of item "ModelRack.app" of container window to {240, 230}
    set position of item "Applications" of container window to {680, 230}
    update without registering applications
    delay 0.5
    close
  end tell
end tell
APPLESCRIPT

sync
sleep 1
hdiutil detach "$MOUNT_DIR" -quiet
trap - EXIT

hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -ov -o "$FINAL_DMG" >/dev/null

if [[ -n "$SIGN_IDENTITY" ]]; then
  codesign --force --timestamp --sign "$SIGN_IDENTITY" "$FINAL_DMG" >/dev/null
fi

if [[ -n "$PROFILE" ]]; then
  xcrun notarytool submit "$FINAL_DMG" --keychain-profile "$PROFILE" --wait
  xcrun stapler staple "$FINAL_DMG"
  xcrun stapler validate "$FINAL_DMG"
  spctl --assess --type open --context context:primary-signature --verbose=4 "$FINAL_DMG"
fi

shasum -a 256 "$FINAL_DMG" > "$FINAL_SUM"
printf 'DMG: %s\n' "$FINAL_DMG"
printf 'Checksum: '
cat "$FINAL_SUM"
